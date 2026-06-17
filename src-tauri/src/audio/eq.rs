use rodio::{source::SeekError, Source};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::resampler::PositionSource;

pub const NUM_BANDS: usize = 10;

/// Center frequencies in Hz for the 10 graphic EQ bands.
pub const BAND_FREQS: [f32; NUM_BANDS] =
    [32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

/// Q factor (~1 octave bandwidth) applied to every peaking band.
const BAND_Q: f32 = 1.41;

pub const MAX_GAIN_DB: f32 = 12.0;

// ---------------------------------------------------------------------------
// Shared EQ state — written by IPC commands, read by the audio thread.
// ---------------------------------------------------------------------------

/// Neutral stereo width (no change). Values >1 widen, <1 narrow, 0 = mono.
pub const STEREO_WIDTH_DEFAULT: f32 = 1.0;
pub const STEREO_WIDTH_MAX: f32 = 3.0;

/// Gain values are stored as raw f32 bits in AtomicU32 so the audio thread
/// can read them without taking a lock on every sample.
pub struct EqState {
    pub bypass: AtomicBool,
    pub gains_db: [AtomicU32; NUM_BANDS],
    /// Stereo expander (mid/side widening) toggle and width factor.
    pub stereo_enabled: AtomicBool,
    pub stereo_width: AtomicU32,
    /// Dynamic EQ: scales the EQ effect by the signal level so boosts/cuts
    /// apply more during quiet passages and ease off when the music is loud.
    pub dynamic_enabled: AtomicBool,
}

impl EqState {
    pub fn new() -> Self {
        Self {
            bypass: AtomicBool::new(false),
            gains_db: std::array::from_fn(|_| AtomicU32::new(0f32.to_bits())),
            stereo_enabled: AtomicBool::new(false),
            stereo_width: AtomicU32::new(STEREO_WIDTH_DEFAULT.to_bits()),
            dynamic_enabled: AtomicBool::new(false),
        }
    }

    pub fn get_gain_db(&self, band: usize) -> f32 {
        f32::from_bits(self.gains_db[band].load(Ordering::Relaxed))
    }

    pub fn set_gain_db(&self, band: usize, gain_db: f32) {
        let clamped = gain_db.clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
        self.gains_db[band].store(clamped.to_bits(), Ordering::Relaxed);
    }

    pub fn get_all_gains(&self) -> [f32; NUM_BANDS] {
        std::array::from_fn(|i| self.get_gain_db(i))
    }

    pub fn is_bypass(&self) -> bool {
        self.bypass.load(Ordering::Relaxed)
    }

    pub fn set_bypass(&self, bypass: bool) {
        self.bypass.store(bypass, Ordering::Relaxed);
    }

    pub fn is_stereo_enabled(&self) -> bool {
        self.stereo_enabled.load(Ordering::Relaxed)
    }

    pub fn set_stereo_enabled(&self, enabled: bool) {
        self.stereo_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn get_stereo_width(&self) -> f32 {
        f32::from_bits(self.stereo_width.load(Ordering::Relaxed))
    }

    pub fn set_stereo_width(&self, width: f32) {
        let clamped = width.clamp(0.0, STEREO_WIDTH_MAX);
        self.stereo_width.store(clamped.to_bits(), Ordering::Relaxed);
    }

    pub fn is_dynamic_enabled(&self) -> bool {
        self.dynamic_enabled.load(Ordering::Relaxed)
    }

    pub fn set_dynamic_enabled(&self, enabled: bool) {
        self.dynamic_enabled.store(enabled, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// Biquad peaking EQ — Audio EQ Cookbook (Robert Bristow-Johnson)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl BiquadCoeffs {
    fn unity() -> Self {
        Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 }
    }

    /// Peaking EQ biquad. Coefficients pre-divided by a0.
    fn peaking_eq(freq_hz: f32, gain_db: f32, q: f32, sample_rate: f32) -> Self {
        if gain_db.abs() < 0.001 {
            return Self::unity();
        }
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq_hz / sample_rate;
        let sin_w0 = w0.sin();
        let cos_w0 = w0.cos();
        let alpha = sin_w0 / (2.0 * q);

        let b0_raw = 1.0 + alpha * a;
        let b1_raw = -2.0 * cos_w0;
        let b2_raw = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1_raw = -2.0 * cos_w0;
        let a2_raw = 1.0 - alpha / a;

        Self {
            b0: b0_raw / a0,
            b1: b1_raw / a0,
            b2: b2_raw / a0,
            a1: a1_raw / a0,
            a2: a2_raw / a0,
        }
    }
}

/// DF-II transposed per-channel filter state.
#[derive(Clone, Copy, Default)]
struct BiquadState {
    z1: f32,
    z2: f32,
}

impl BiquadState {
    #[inline(always)]
    fn process(&mut self, x: f32, c: &BiquadCoeffs) -> f32 {
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        y
    }

    fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

// ---------------------------------------------------------------------------
// EqSource — rodio Source wrapper
// ---------------------------------------------------------------------------

/// Wraps `PositionSource` and applies a 10-band peaking EQ in series.
///
/// Band coefficients are recomputed at frame boundaries when the shared
/// `EqState` gains change — never per-sample. Bypass is checked per-sample
/// via a single atomic bool.
pub struct EqSource {
    inner: PositionSource,
    eq_state: Arc<EqState>,
    sample_rate: f32,
    channels: u16,
    total_duration: Option<Duration>,

    coeffs: [BiquadCoeffs; NUM_BANDS],
    /// One set of per-band biquad states per audio channel (supports up to 8).
    states: Vec<[BiquadState; NUM_BANDS]>,
    last_gains: [f32; NUM_BANDS],
    channel_idx: u16,
    /// Per-channel envelope follower for the dynamic-EQ blend.
    env: Vec<f32>,
    dyn_attack: f32,
    dyn_release: f32,
}

// Dynamic-EQ shaping constants (linear sample magnitude domain).
const DYN_THRESH: f32 = 0.2; // level at which EQ begins easing off
const DYN_RANGE: f32 = 0.6; // span over which it ramps to minimum
const DYN_DEPTH: f32 = 0.7; // max fraction of EQ effect removed when loud

/// One-pole smoothing coefficient for a given time constant.
fn smoothing_coef(time_secs: f32, sample_rate: f32) -> f32 {
    if time_secs <= 0.0 || sample_rate <= 0.0 {
        return 0.0;
    }
    (-1.0 / (time_secs * sample_rate)).exp()
}

impl EqSource {
    pub fn new(inner: PositionSource, eq_state: Arc<EqState>) -> Self {
        let channels = inner.channels();
        let sample_rate = inner.sample_rate() as f32;
        let total_duration = inner.total_duration();
        let initial_gains = eq_state.get_all_gains();
        let coeffs = std::array::from_fn(|i| {
            BiquadCoeffs::peaking_eq(BAND_FREQS[i], initial_gains[i], BAND_Q, sample_rate)
        });
        let states = vec![[BiquadState::default(); NUM_BANDS]; channels.max(1) as usize];

        Self {
            inner,
            eq_state,
            sample_rate,
            channels,
            total_duration,
            coeffs,
            states,
            last_gains: initial_gains,
            channel_idx: 0,
            env: vec![0.0; channels.max(1) as usize],
            dyn_attack: smoothing_coef(0.005, sample_rate),
            dyn_release: smoothing_coef(0.15, sample_rate),
        }
    }

    /// Re-read gains and recompute biquad coefficients if anything changed.
    /// Called at frame boundary (channel_idx == 0) to amortize the cost.
    fn refresh_coeffs_if_needed(&mut self) {
        let current = self.eq_state.get_all_gains();
        if current == self.last_gains {
            return;
        }
        for (i, &freq) in BAND_FREQS.iter().enumerate() {
            self.coeffs[i] =
                BiquadCoeffs::peaking_eq(freq, current[i], BAND_Q, self.sample_rate);
        }
        self.last_gains = current;
    }
}

impl Iterator for EqSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.channel_idx == 0 {
            self.refresh_coeffs_if_needed();
        }

        let sample = self.inner.next()?;

        if self.eq_state.is_bypass() {
            self.channel_idx = (self.channel_idx + 1) % self.channels.max(1);
            return Some(sample);
        }

        let ch = self.channel_idx as usize % self.states.len();
        self.channel_idx = (self.channel_idx + 1) % self.channels.max(1);

        let ch_states = &mut self.states[ch];
        let mut s = sample;
        for band in 0..NUM_BANDS {
            s = ch_states[band].process(s, &self.coeffs[band]);
        }

        // Dynamic EQ: blend the EQ'd signal back toward dry as the level rises,
        // so EQ shaping is strongest in quiet passages and gentlest when loud.
        if self.eq_state.is_dynamic_enabled() {
            let mag = s.abs();
            let env = &mut self.env[ch];
            let coef = if mag > *env { self.dyn_attack } else { self.dyn_release };
            *env = mag + coef * (*env - mag);
            let over = ((*env - DYN_THRESH) / DYN_RANGE).clamp(0.0, 1.0);
            let factor = 1.0 - over * DYN_DEPTH;
            s = sample + factor * (s - sample);
        }

        Some(s)
    }
}

impl Source for EqSource {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate as u32
    }
    fn total_duration(&self) -> Option<Duration> {
        self.total_duration
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)?;
        // Reset biquad delay-line state to prevent clicks at the seek point.
        for ch_states in &mut self.states {
            for state in ch_states.iter_mut() {
                state.reset();
            }
        }
        for e in &mut self.env {
            *e = 0.0;
        }
        self.channel_idx = 0;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// StereoExpander — mid/side stereo widening
// ---------------------------------------------------------------------------

/// Widens (or narrows) the stereo image via mid/side processing. Only active
/// for 2-channel sources with the expander enabled; otherwise it forwards
/// samples untouched. Placed after `EqSource` in the chain.
pub struct StereoExpander {
    inner: EqSource,
    eq_state: Arc<EqState>,
    channels: u16,
    sample_rate: u32,
    total_duration: Option<Duration>,
    /// Processed right-channel sample held back until the next `next()` call.
    pending_right: Option<f32>,
}

impl StereoExpander {
    pub fn new(inner: EqSource, eq_state: Arc<EqState>) -> Self {
        let channels = inner.channels();
        let sample_rate = inner.sample_rate();
        let total_duration = inner.total_duration();
        Self {
            inner,
            eq_state,
            channels,
            sample_rate,
            total_duration,
            pending_right: None,
        }
    }
}

impl Iterator for StereoExpander {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if let Some(r) = self.pending_right.take() {
            return Some(r);
        }

        let left = self.inner.next()?;

        if self.channels != 2 || !self.eq_state.is_stereo_enabled() {
            return Some(left);
        }

        // Pull the matching right sample; if the source just ended, emit left.
        let right = match self.inner.next() {
            Some(r) => r,
            None => return Some(left),
        };

        let width = self.eq_state.get_stereo_width();
        let mid = 0.5 * (left + right);
        let side = 0.5 * (left - right) * width;
        let out_l = (mid + side).clamp(-1.0, 1.0);
        let out_r = (mid - side).clamp(-1.0, 1.0);

        self.pending_right = Some(out_r);
        Some(out_l)
    }
}

impl Source for StereoExpander {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.total_duration
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.pending_right = None;
        self.inner.try_seek(pos)
    }
}
