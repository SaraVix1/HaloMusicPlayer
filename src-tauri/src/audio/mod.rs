pub mod eq;
pub mod local_input;
pub mod media_controls;
pub mod resampler;
pub mod spectrum;

use eq::EqSource;
use halo_core::audio_event::AudioEvent;
use halo_core::media_input::MediaInput;
use local_input::LocalFsInput;
use resampler::{PositionSource, ResampledSource, device_sample_rate};
use rodio::{Decoder, OutputStream, Sink, Source};
use serde::Serialize;
use spectrum::{SpectrumBuffer, SpectrumTap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Remaining audio threshold at which the Worker emits `AudioEvent::NearEnd`.
/// Must be larger than the maximum crossfade window (12 000 ms) plus the
/// 250 ms CROSSFADE_TRIGGER_BUFFER so the app layer always has time to act.
const NEAR_END_THRESHOLD_MS: u64 = 15_000;

#[derive(Clone, Copy, Serialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackStatus {
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Serialize)]
pub struct PlayerState {
    pub status: PlaybackStatus,
    pub track_id: Option<i64>,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub volume: f32,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            track_id: None,
            position_ms: 0,
            duration_ms: None,
            volume: 0.75,
        }
    }
}

enum Command {
    Load {
        track_id: i64,
        path: PathBuf,
        reply: Sender<Result<(), String>>,
    },
    LoadAndCrossfade {
        track_id: i64,
        path: PathBuf,
        fade_ms: u64,
        reply: Sender<Result<(), String>>,
    },
    Pause,
    Resume,
    Stop,
    Seek {
        position_ms: u64,
        reply: Sender<Result<(), String>>,
    },
    SetVolume(f32),
    /// Reopen the output stream on the current system-default device, resuming
    /// the current track in place. Sent when the OS default output changes.
    RebuildOutput {
        reply: Sender<Result<(), String>>,
    },
}

struct CrossfadeState {
    started_at: Instant,
    duration: Duration,
}

struct Worker {
    sink: Sink,
    fading_out_sink: Option<Sink>,
    crossfade: Option<CrossfadeState>,
    track_id: Option<i64>,
    /// Path of the currently-loaded track, kept so the worker can re-open and
    /// resume it after the output device is rebuilt on a different device.
    current_path: Option<PathBuf>,
    duration_ms: Option<u64>,
    /// Seek position at the time of the last seek command or track load.
    seek_base_ms: u64,
    /// Channel count of the currently playing source (for position math).
    source_channels: u16,
    /// Sample counter shared with the current `PositionSource`.
    position_counter: Arc<AtomicU64>,
    /// True once `AudioEvent::NearEnd` has been sent for the current track.
    near_end_sent: bool,
    volume: f32,
    state: Arc<Mutex<PlayerState>>,
    output_sample_rate: u32,
    event_tx: Sender<AudioEvent>,
    eq_state: Arc<eq::EqState>,
    spectrum_buf: SpectrumBuffer,
}

impl Worker {
    fn current_position_ms(&self) -> u64 {
        let samples = self.position_counter.load(Ordering::Relaxed);
        self.seek_base_ms
            + PositionSource::ms_from_samples(
                samples,
                self.source_channels,
                self.output_sample_rate,
            )
    }

    fn status(&self) -> PlaybackStatus {
        if self.track_id.is_none() {
            PlaybackStatus::Stopped
        } else if self.sink.is_paused() {
            PlaybackStatus::Paused
        } else {
            PlaybackStatus::Playing
        }
    }

    fn write_state(&self) {
        if let Ok(mut s) = self.state.lock() {
            s.status = self.status();
            s.track_id = self.track_id;
            s.position_ms = self.current_position_ms();
            s.duration_ms = self.duration_ms;
            s.volume = self.volume;
        }
    }

    fn is_finished(&self) -> bool {
        self.track_id.is_some()
            && self.sink.empty()
            && !self.sink.is_paused()
            && self.crossfade.is_none()
    }

    fn tick_crossfade(&mut self) {
        let Some(cf) = self.crossfade.as_ref() else {
            return;
        };
        let elapsed = cf.started_at.elapsed();
        let progress = if cf.duration.is_zero() {
            1.0
        } else {
            (elapsed.as_secs_f32() / cf.duration.as_secs_f32()).clamp(0.0, 1.0)
        };
        self.sink.set_volume(self.volume * progress);
        if let Some(out) = &self.fading_out_sink {
            out.set_volume(self.volume * (1.0 - progress));
        }
        if progress >= 1.0 {
            self.sink.set_volume(self.volume);
            self.fading_out_sink = None;
            self.crossfade = None;
        }
    }

    fn reset_track_state(&mut self) {
        self.track_id = None;
        self.current_path = None;
        self.duration_ms = None;
        self.seek_base_ms = 0;
        self.source_channels = 0;
        self.position_counter = Arc::new(AtomicU64::new(0));
        self.near_end_sent = false;
        if let Ok(mut buf) = self.spectrum_buf.lock() {
            buf.clear();
        }
    }
}

// ---------------------------------------------------------------------------
// AudioBackend port
// ---------------------------------------------------------------------------

/// Port for audio output. `PlayerHandle` is the desktop (rodio) implementation.
/// A fake/mock backend can be injected in tests; a native backend (AAudio, AVAudioEngine)
/// can replace it in a later phase without touching command logic.
#[allow(dead_code)]
pub trait AudioBackend: Send + Sync {
    fn load_and_play(&self, track_id: i64, path: PathBuf) -> Result<(), String>;
    fn load_and_crossfade(&self, track_id: i64, path: PathBuf, fade_ms: u64) -> Result<(), String>;
    fn pause(&self);
    fn resume(&self);
    fn stop(&self);
    fn seek(&self, position_ms: u64) -> Result<(), String>;
    fn set_volume(&self, volume: f32);
    fn snapshot(&self) -> PlayerState;
    /// Drain all pending `AudioEvent`s since the last call.
    /// Replaces the old `take_finished_flag` + `is_crossfading` poll pair.
    fn drain_events(&self) -> Vec<AudioEvent>;
}

// ---------------------------------------------------------------------------
// PlayerHandle
// ---------------------------------------------------------------------------

pub struct PlayerHandle {
    cmd_tx: Mutex<Sender<Command>>,
    state: Arc<Mutex<PlayerState>>,
    event_rx: Mutex<mpsc::Receiver<AudioEvent>>,
    pub eq_state: Arc<eq::EqState>,
    pub spectrum_buf: SpectrumBuffer,
    /// Output sample rate captured at startup; used for Goertzel bin math.
    spectrum_sample_rate: u32,
}

impl PlayerHandle {
    fn send(&self, cmd: Command) -> Result<(), String> {
        self.cmd_tx
            .lock()
            .map_err(|e| e.to_string())?
            .send(cmd)
            .map_err(|e| e.to_string())
    }
}

impl PlayerHandle {
    pub fn new(initial_volume: Option<f32>, eq_state: Arc<eq::EqState>) -> Result<Self, String> {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();
        let state = Arc::new(Mutex::new(PlayerState {
            volume: initial_volume.unwrap_or(0.75),
            ..PlayerState::default()
        }));
        let (event_tx, event_rx) = mpsc::channel::<AudioEvent>();

        let state_for_thread = Arc::clone(&state);
        let initial_vol = initial_volume.unwrap_or(0.75);
        let output_sample_rate = device_sample_rate();
        let eq_state_for_thread = Arc::clone(&eq_state);

        let spectrum_buf = spectrum::new_buffer();
        let spectrum_buf_for_worker = Arc::clone(&spectrum_buf);

        let (init_tx, init_rx) = mpsc::channel::<Result<(), String>>();

        std::thread::Builder::new()
            .name("halo-audio".into())
            .spawn(move || {
                let (mut _stream, mut handle) = match OutputStream::try_default() {
                    Ok(t) => t,
                    Err(e) => {
                        let _ = init_tx.send(Err(e.to_string()));
                        return;
                    }
                };
                let sink = match Sink::try_new(&handle) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = init_tx.send(Err(e.to_string()));
                        return;
                    }
                };
                sink.set_volume(initial_vol);
                sink.pause();
                let _ = init_tx.send(Ok(()));

                let mut worker = Worker {
                    sink,
                    fading_out_sink: None,
                    crossfade: None,
                    track_id: None,
                    current_path: None,
                    duration_ms: None,
                    seek_base_ms: 0,
                    source_channels: 0,
                    position_counter: Arc::new(AtomicU64::new(0)),
                    near_end_sent: false,
                    volume: initial_vol,
                    state: state_for_thread,
                    output_sample_rate,
                    event_tx,
                    eq_state: eq_state_for_thread,
                    spectrum_buf: spectrum_buf_for_worker,
                };

                loop {
                    let cmd = match cmd_rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(c) => Some(c),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };

                    if let Some(cmd) = cmd {
                        match cmd {
                            Command::Load { track_id, path, reply } => {
                                let result = LocalFsInput::open(&path)
                                    .and_then(|i| load_into_worker(&mut worker, &handle, track_id, i, 0));
                                if result.is_ok() {
                                    worker.current_path = Some(path);
                                }
                                let _ = reply.send(result);
                            }
                            Command::LoadAndCrossfade { track_id, path, fade_ms, reply } => {
                                let result = LocalFsInput::open(&path)
                                    .and_then(|i| start_crossfade(&mut worker, &handle, track_id, i, fade_ms));
                                if result.is_ok() {
                                    worker.current_path = Some(path);
                                }
                                let _ = reply.send(result);
                            }
                            Command::RebuildOutput { reply } => {
                                let result = rebuild_output(&mut worker, &mut _stream, &mut handle);
                                let _ = reply.send(result);
                            }
                            Command::Pause => {
                                worker.sink.pause();
                                if let Some(out) = &worker.fading_out_sink {
                                    out.pause();
                                }
                            }
                            Command::Resume => {
                                if worker.track_id.is_some() {
                                    worker.sink.play();
                                    if let Some(out) = &worker.fading_out_sink {
                                        out.play();
                                    }
                                }
                            }
                            Command::Stop => {
                                worker.sink.stop();
                                if let Some(out) = worker.fading_out_sink.take() {
                                    out.stop();
                                }
                                worker.crossfade = None;
                                worker.reset_track_state();
                            }
                            Command::Seek { position_ms, reply } => {
                                let result = if worker.track_id.is_none() {
                                    Ok(())
                                } else {
                                    let was_paused = worker.sink.is_paused();
                                    if worker.sink.try_seek(Duration::from_millis(position_ms)).is_ok() {
                                        // Fast path: native seek (seekable formats or FLAC with SEEKTABLE).
                                        // PositionSource::try_seek already reset the counter to 0.
                                        worker.seek_base_ms = position_ms;
                                        if let Some(d) = worker.duration_ms {
                                            if d.saturating_sub(position_ms) > NEAR_END_THRESHOLD_MS {
                                                worker.near_end_sent = false;
                                            }
                                        }
                                        Ok(())
                                    } else if let (Some(tid), Some(path)) =
                                        (worker.track_id, worker.current_path.clone())
                                    {
                                        // Slow path: reload the decoder and use skip_duration to
                                        // discard samples up to position_ms before PositionSource.
                                        // Needed for FLAC files without a SEEKTABLE block.
                                        // seek_base_ms = position_ms is set inside load_into_worker;
                                        // the counter starts at 0 after the skip, so position tracking
                                        // is correct. The audio thread stalls briefly on the first
                                        // callback while discarding frames (typically < 0.5 s).
                                        match LocalFsInput::open(&path)
                                            .and_then(|i| load_into_worker(&mut worker, &handle, tid, i, position_ms))
                                        {
                                            Ok(_) => {
                                                worker.current_path = Some(path);
                                                if was_paused {
                                                    worker.sink.pause();
                                                }
                                                Ok(())
                                            }
                                            Err(e) => Err(format!("seek: {e}")),
                                        }
                                    } else {
                                        Err("seek: no current track".to_string())
                                    }
                                };
                                let _ = reply.send(result);
                            }
                            Command::SetVolume(v) => {
                                let clamped = v.clamp(0.0, 1.0);
                                worker.volume = clamped;
                                if worker.crossfade.is_none() {
                                    worker.sink.set_volume(clamped);
                                }
                            }
                        }
                    }

                    worker.tick_crossfade();

                    // Emit NearEnd once per track when remaining drops below threshold.
                    if !worker.near_end_sent {
                        if let (Some(tid), Some(duration_ms)) = (worker.track_id, worker.duration_ms) {
                            if worker.status() == PlaybackStatus::Playing {
                                let pos = worker.current_position_ms();
                                let remaining = duration_ms.saturating_sub(pos);
                                if remaining <= NEAR_END_THRESHOLD_MS && remaining > 0 {
                                    let _ = worker.event_tx.send(AudioEvent::NearEnd {
                                        track_id: tid,
                                        remaining_ms: remaining,
                                    });
                                    worker.near_end_sent = true;
                                }
                            }
                        }
                    }

                    if worker.is_finished() {
                        if let Some(tid) = worker.track_id {
                            let _ = worker.event_tx.send(AudioEvent::TrackFinished { track_id: tid });
                        }
                        worker.sink.stop();
                        worker.reset_track_state();
                    }

                    worker.write_state();
                }
            })
            .map_err(|e| e.to_string())?;

        init_rx.recv().map_err(|e| e.to_string())??;

        Ok(Self {
            cmd_tx: Mutex::new(cmd_tx),
            state,
            event_rx: Mutex::new(event_rx),
            eq_state,
            spectrum_buf,
            spectrum_sample_rate: output_sample_rate,
        })
    }

    pub fn snapshot(&self) -> PlayerState {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Returns 24 frequency-band magnitudes in [0, 1] for the current frame.
    /// Returns all-zeros when not playing so bars decay naturally on the frontend.
    pub fn compute_spectrum(&self) -> Vec<f32> {
        if self.snapshot().status != PlaybackStatus::Playing {
            return vec![0.0; spectrum::NUM_BANDS];
        }
        spectrum::compute_bands(&self.spectrum_buf, self.spectrum_sample_rate)
    }

    pub fn drain_events(&self) -> Vec<AudioEvent> {
        let Ok(rx) = self.event_rx.lock() else { return vec![] };
        let mut events = Vec::new();
        while let Ok(e) = rx.try_recv() {
            events.push(e);
        }
        events
    }

    pub fn load_and_play(&self, track_id: i64, path: PathBuf) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.send(Command::Load { track_id, path, reply: tx })?;
        rx.recv().map_err(|e| e.to_string())?
    }

    pub fn load_and_crossfade(&self, track_id: i64, path: PathBuf, fade_ms: u64) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.send(Command::LoadAndCrossfade { track_id, path, fade_ms, reply: tx })?;
        rx.recv().map_err(|e| e.to_string())?
    }

    pub fn pause(&self) {
        let _ = self.send(Command::Pause);
    }

    pub fn resume(&self) {
        let _ = self.send(Command::Resume);
    }

    pub fn stop(&self) {
        let _ = self.send(Command::Stop);
    }

    pub fn seek(&self, position_ms: u64) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.send(Command::Seek { position_ms, reply: tx })?;
        rx.recv().map_err(|e| e.to_string())?
    }

    pub fn set_volume(&self, volume: f32) {
        let _ = self.send(Command::SetVolume(volume));
    }

    /// Reopen the output on the current system-default device, resuming the
    /// current track in place. Called when the OS default output changes.
    pub fn rebuild_output(&self) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.send(Command::RebuildOutput { reply: tx })?;
        rx.recv().map_err(|e| e.to_string())?
    }
}

impl AudioBackend for PlayerHandle {
    fn load_and_play(&self, track_id: i64, path: PathBuf) -> Result<(), String> {
        self.load_and_play(track_id, path)
    }
    fn load_and_crossfade(&self, track_id: i64, path: PathBuf, fade_ms: u64) -> Result<(), String> {
        self.load_and_crossfade(track_id, path, fade_ms)
    }
    fn pause(&self) { self.pause() }
    fn resume(&self) { self.resume() }
    fn stop(&self) { self.stop() }
    fn seek(&self, position_ms: u64) -> Result<(), String> { self.seek(position_ms) }
    fn set_volume(&self, volume: f32) { self.set_volume(volume) }
    fn snapshot(&self) -> PlayerState { self.snapshot() }
    fn drain_events(&self) -> Vec<AudioEvent> { self.drain_events() }
}

// ---------------------------------------------------------------------------
// Worker helpers
// ---------------------------------------------------------------------------

fn decode_resampled(
    input: Box<dyn MediaInput>,
    output_rate: u32,
    counter: Arc<AtomicU64>,
    eq_state: Arc<eq::EqState>,
    spectrum_buf: SpectrumBuffer,
    skip_ms: u64,
) -> Result<(SpectrumTap<eq::StereoExpander>, Option<u64>), String> {
    let decoder = Decoder::new(input).map_err(|e| format!("decode: {e}"))?;
    let duration_ms = decoder.total_duration().map(|d| d.as_millis() as u64);
    // Apply the skip BEFORE PositionSource so the counter only counts post-seek samples.
    // skip_duration tries try_seek first (fast path); for unseekable sources like FLAC
    // without a SEEKTABLE it falls back to decoding and discarding (brief stall on the
    // first audio callback, but the position is exact and tracking is correct).
    let converted: Box<dyn Source<Item = f32> + Send> = if skip_ms > 0 {
        Box::new(decoder.convert_samples::<f32>().skip_duration(Duration::from_millis(skip_ms)))
    } else {
        Box::new(decoder.convert_samples::<f32>())
    };
    let resampled = ResampledSource::new(converted, output_rate);
    let position = PositionSource::new(resampled, counter);
    let eq = EqSource::new(position, Arc::clone(&eq_state));
    let expander = eq::StereoExpander::new(eq, eq_state);
    Ok((SpectrumTap::new(expander, spectrum_buf), duration_ms))
}

/// Reopen the output stream on the current system-default device, then resume
/// the in-progress track at its current position (preserving paused state and
/// volume). The new device may run at a different sample rate, so the track is
/// re-decoded fresh against `device_sample_rate()`.
fn rebuild_output(
    worker: &mut Worker,
    stream: &mut OutputStream,
    handle: &mut rodio::OutputStreamHandle,
) -> Result<(), String> {
    // Snapshot playback state before tearing down the old device.
    let track_id = worker.track_id;
    let path = worker.current_path.clone();
    let pos = worker.current_position_ms();
    let was_paused = worker.sink.is_paused();
    let volume = worker.volume;

    // Open the new system-default device first; bail without disrupting
    // playback if it fails (e.g. transient "no default device" during switch).
    let (new_stream, new_handle) =
        OutputStream::try_default().map_err(|e| format!("reopen output: {e}"))?;
    let new_rate = device_sample_rate();

    // Tear down sinks bound to the old stream, which is about to be dropped.
    worker.sink.stop();
    if let Some(out) = worker.fading_out_sink.take() {
        out.stop();
    }
    worker.crossfade = None;

    *stream = new_stream;
    *handle = new_handle;
    worker.output_sample_rate = new_rate;

    match (track_id, path) {
        (Some(tid), Some(path)) => {
            let input = LocalFsInput::open(&path)?;
            if let Ok(mut buf) = worker.spectrum_buf.lock() { buf.clear(); }
            load_into_worker(worker, handle, tid, input, 0)?;
            worker.current_path = Some(path);
            if pos > 0 && worker.sink.try_seek(Duration::from_millis(pos)).is_ok() {
                worker.seek_base_ms = pos;
            }
            if was_paused {
                worker.sink.pause();
            }
        }
        _ => {
            // Idle: give the worker a fresh paused sink on the new device.
            let sink = Sink::try_new(handle).map_err(|e| e.to_string())?;
            sink.set_volume(volume);
            sink.pause();
            worker.sink = sink;
        }
    }
    Ok(())
}

fn load_into_worker(
    worker: &mut Worker,
    handle: &rodio::OutputStreamHandle,
    track_id: i64,
    input: Box<dyn MediaInput>,
    skip_ms: u64,
) -> Result<(), String> {
    let counter = Arc::new(AtomicU64::new(0));
    let eq = Arc::clone(&worker.eq_state);
    let (source, duration_ms) = decode_resampled(input, worker.output_sample_rate, counter.clone(), eq, Arc::clone(&worker.spectrum_buf), skip_ms)?;
    let channels = source.channels();

    let new_sink = Sink::try_new(handle).map_err(|e| e.to_string())?;
    new_sink.set_volume(worker.volume);
    new_sink.append(source);

    worker.sink.stop();
    if let Some(out) = worker.fading_out_sink.take() {
        out.stop();
    }
    worker.crossfade = None;
    worker.sink = new_sink;
    worker.track_id = Some(track_id);
    worker.duration_ms = duration_ms;
    worker.seek_base_ms = skip_ms;
    worker.source_channels = channels;
    worker.position_counter = counter;
    worker.near_end_sent = false;
    Ok(())
}

fn start_crossfade(
    worker: &mut Worker,
    handle: &rodio::OutputStreamHandle,
    track_id: i64,
    input: Box<dyn MediaInput>,
    fade_ms: u64,
) -> Result<(), String> {
    if fade_ms == 0 || worker.track_id.is_none() {
        return load_into_worker(worker, handle, track_id, input, 0);
    }

    let counter = Arc::new(AtomicU64::new(0));
    let eq = Arc::clone(&worker.eq_state);
    let (source, duration_ms) = decode_resampled(input, worker.output_sample_rate, counter.clone(), eq, Arc::clone(&worker.spectrum_buf), 0)?;
    let channels = source.channels();

    let new_sink = Sink::try_new(handle).map_err(|e| e.to_string())?;
    new_sink.set_volume(0.0);
    new_sink.append(source);
    if worker.sink.is_paused() {
        new_sink.pause();
    }

    // Never keep more than two sinks at once.
    if let Some(prev_out) = worker.fading_out_sink.take() {
        prev_out.stop();
    }

    let old_sink = std::mem::replace(&mut worker.sink, new_sink);
    worker.fading_out_sink = Some(old_sink);

    worker.crossfade = Some(CrossfadeState {
        started_at: Instant::now(),
        duration: Duration::from_millis(fade_ms),
    });
    worker.track_id = Some(track_id);
    worker.duration_ms = duration_ms;
    worker.seek_base_ms = 0;
    worker.source_channels = channels;
    worker.position_counter = counter;
    worker.near_end_sent = false;
    Ok(())
}
