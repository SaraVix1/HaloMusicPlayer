use rodio::{source::SeekError, Source};
use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const NUM_BANDS: usize = 24;
const WINDOW_SIZE: usize = 1024;
const BUFFER_CAP: usize = 2048;
const MIN_FREQ: f32 = 60.0;
const MAX_FREQ: f32 = 16_000.0;
/// Empirical gain applied before sqrt-scaling. Tune up if bars look too quiet.
const GAIN: f32 = 12.0;

pub type SpectrumBuffer = Arc<Mutex<VecDeque<f32>>>;

pub fn new_buffer() -> SpectrumBuffer {
    Arc::new(Mutex::new(VecDeque::with_capacity(BUFFER_CAP + 1)))
}

// ---------------------------------------------------------------------------
// SpectrumTap — sits at the end of the audio chain, taps mono samples into
// a shared ring buffer without blocking the audio callback.
// ---------------------------------------------------------------------------

pub struct SpectrumTap<S> {
    inner: S,
    buffer: SpectrumBuffer,
    /// Number of channels of the inner source (captured once at construction).
    ch_count: u16,
    /// Which channel sample we are currently on (0-indexed).
    ch_idx: u16,
    /// Accumulator for mono mixing across channels.
    acc: f32,
}

impl<S: Source<Item = f32>> SpectrumTap<S> {
    pub fn new(inner: S, buffer: SpectrumBuffer) -> Self {
        let ch_count = inner.channels().max(1);
        Self { inner, buffer, ch_count, ch_idx: 0, acc: 0.0 }
    }
}

impl<S: Source<Item = f32>> Iterator for SpectrumTap<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        self.acc += sample;
        self.ch_idx += 1;
        if self.ch_idx >= self.ch_count {
            let mono = self.acc / self.ch_count as f32;
            self.acc = 0.0;
            self.ch_idx = 0;
            // try_lock — drop the sample rather than blocking the audio thread.
            if let Ok(mut buf) = self.buffer.try_lock() {
                if buf.len() >= BUFFER_CAP {
                    buf.pop_front();
                }
                buf.push_back(mono);
            }
        }
        Some(sample)
    }
}

impl<S: Source<Item = f32>> Source for SpectrumTap<S> {
    fn current_frame_len(&self) -> Option<usize> { self.inner.current_frame_len() }
    fn channels(&self) -> u16 { self.inner.channels() }
    fn sample_rate(&self) -> u32 { self.inner.sample_rate() }
    fn total_duration(&self) -> Option<Duration> { self.inner.total_duration() }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)?;
        self.ch_idx = 0;
        self.acc = 0.0;
        // Clear stale samples so the seek position isn't smeared into the next frame.
        if let Ok(mut buf) = self.buffer.lock() {
            buf.clear();
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// compute_bands — Goertzel-based frequency analysis, no external dependency.
// Returns NUM_BANDS magnitudes in [0, 1], log-spaced from MIN_FREQ to MAX_FREQ.
// ---------------------------------------------------------------------------

pub fn compute_bands(buffer: &SpectrumBuffer, sample_rate: u32) -> Vec<f32> {
    // Snapshot the most recent samples without blocking long.
    let samples: Vec<f32> = {
        let Ok(guard) = buffer.try_lock() else {
            return vec![0.0; NUM_BANDS];
        };
        if guard.len() < 64 {
            return vec![0.0; NUM_BANDS];
        }
        let n = guard.len().min(WINDOW_SIZE);
        // Take the newest n samples (ring-buffer tail).
        guard.iter().rev().take(n).copied().collect::<Vec<_>>()
            .into_iter().rev().collect()
    };

    let n = samples.len();
    let sr = sample_rate as f32;

    // Apply a Hamming window to reduce spectral leakage.
    let windowed: Vec<f32> = samples.iter().enumerate().map(|(i, &s)| {
        let w = 0.54 - 0.46 * (2.0 * PI * i as f32 / (n - 1).max(1) as f32).cos();
        s * w
    }).collect();

    (0..NUM_BANDS).map(|b| {
        // Logarithmically-spaced target frequency for this band.
        let freq = MIN_FREQ * (MAX_FREQ / MIN_FREQ).powf(b as f32 / (NUM_BANDS - 1) as f32);

        // Goertzel algorithm: compute DFT magnitude at the nearest bin.
        let k = (n as f32 * freq / sr).round().clamp(1.0, n as f32 / 2.0 - 1.0);
        let omega = 2.0 * PI * k / n as f32;
        let coeff = 2.0 * omega.cos();

        let mut s1 = 0.0f32; // s[n-1]
        let mut s2 = 0.0f32; // s[n-2]
        for &x in &windowed {
            let s0 = coeff * s1 - s2 + x;
            s2 = s1;
            s1 = s0;
        }

        // Magnitude = sqrt(s1² + s2² − coeff·s1·s2), normalised by N/2.
        let raw = (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0).sqrt();
        let normalised = (raw / (n as f32 / 2.0)) * GAIN;
        // sqrt-scale compresses the dynamic range for a more visually balanced display.
        normalised.sqrt().clamp(0.0, 1.0)
    }).collect()
}
