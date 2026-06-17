use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use rodio::{Source, source::SeekError};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

const CHUNK_FRAMES: usize = 1024;

/// Returns the default output device's sample rate via cpal, falling back to 48 000.
pub fn device_sample_rate() -> u32 {
    use cpal::traits::{DeviceTrait, HostTrait};
    cpal::default_host()
        .default_output_device()
        .and_then(|d| d.default_output_config().ok())
        .map(|c| c.sample_rate().0)
        .unwrap_or(48_000)
}

/// Wraps any `f32` rodio `Source` with a high-quality sinc resampler (rubato `SincFixedIn`).
/// When input and output rates already match the inner source is forwarded unchanged so
/// there is no performance penalty on same-rate files.
pub struct ResampledSource {
    inner: Box<dyn Source<Item = f32> + Send>,
    channels: u16,
    output_rate: u32,
    total_duration: Option<Duration>,

    resampler: Option<SincFixedIn<f32>>,

    // Per-channel input staging buffers; length == CHUNK_FRAMES while filling.
    in_buf: Vec<Vec<f32>>,

    // Interleaved output ready to be consumed.
    out_queue: VecDeque<f32>,

    exhausted: bool,
}

impl ResampledSource {
    pub fn new<S>(source: S, output_rate: u32) -> Self
    where
        S: Source<Item = f32> + Send + 'static,
    {
        let channels = source.channels();
        let source_rate = source.sample_rate();
        let total_duration = source.total_duration();

        let resampler = if source_rate != output_rate {
            let params = SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: 128,
                window: WindowFunction::BlackmanHarris2,
            };
            SincFixedIn::<f32>::new(
                output_rate as f64 / source_rate as f64,
                2.0,
                params,
                CHUNK_FRAMES,
                channels as usize,
            )
            .ok()
        } else {
            None
        };

        let in_buf = vec![Vec::with_capacity(CHUNK_FRAMES); channels as usize];

        Self {
            inner: Box::new(source),
            channels,
            output_rate,
            total_duration,
            resampler,
            in_buf,
            out_queue: VecDeque::new(),
            exhausted: false,
        }
    }

    /// Pull one chunk from inner, resample it, push results into `out_queue`.
    /// Returns `false` only when the source is exhausted and the queue is empty.
    fn refill(&mut self) -> bool {
        if self.exhausted {
            return !self.out_queue.is_empty();
        }

        let resampler = match &mut self.resampler {
            None => return false,
            Some(r) => r,
        };

        let need = resampler.input_frames_next();
        let ch = self.channels as usize;

        for buf in &mut self.in_buf {
            buf.clear();
        }

        let mut frames_read = 0usize;
        'read: for _ in 0..need {
            for c in 0..ch {
                match self.inner.next() {
                    Some(s) => self.in_buf[c].push(s),
                    None => {
                        self.exhausted = true;
                        break 'read;
                    }
                }
            }
            if !self.exhausted {
                frames_read += 1;
            }
        }

        if frames_read == 0 {
            return !self.out_queue.is_empty();
        }

        for buf in &mut self.in_buf {
            buf.truncate(frames_read);
        }

        let result = if self.exhausted || frames_read < need {
            resampler.process_partial(Some(&self.in_buf), None)
        } else {
            resampler.process(&self.in_buf, None)
        };

        if let Ok(out) = result {
            if let Some(first) = out.first() {
                let n = first.len();
                for f in 0..n {
                    for c in 0..ch {
                        self.out_queue.push_back(out[c][f]);
                    }
                }
            }
        }

        !self.out_queue.is_empty()
    }
}

impl Iterator for ResampledSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.resampler.is_none() {
            return self.inner.next();
        }
        if self.out_queue.is_empty() && !self.refill() {
            return None;
        }
        self.out_queue.pop_front()
    }
}

impl Source for ResampledSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    /// Report the output rate so rodio's mixer sees no rate mismatch and skips
    /// its own linear-interpolation pass entirely.
    fn sample_rate(&self) -> u32 {
        self.output_rate
    }

    /// Duration is a time quantity — it is unaffected by sample-rate conversion.
    fn total_duration(&self) -> Option<Duration> {
        self.total_duration
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)?;
        // Flush in-flight state so we don't mix pre- and post-seek samples.
        self.out_queue.clear();
        for buf in &mut self.in_buf {
            buf.clear();
        }
        if let Some(r) = &mut self.resampler {
            r.reset();
        }
        self.exhausted = false;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PositionSource — sample-clock position tracking
// ---------------------------------------------------------------------------

/// Wraps a `ResampledSource` and counts every sample emitted via `next()`.
/// The `Worker` holds a clone of `samples_played` to read position at any time:
///   position_ms = seek_base_ms + (samples_played / channels / rate * 1000)
/// The counter is reset to 0 inside `try_seek`, so the Worker only needs to
/// update `seek_base_ms` after issuing a seek command.
pub struct PositionSource {
    inner: ResampledSource,
    samples_played: Arc<AtomicU64>,
    channels: u16,
    output_rate: u32,
    total_duration: Option<Duration>,
}

impl PositionSource {
    pub fn new(inner: ResampledSource, counter: Arc<AtomicU64>) -> Self {
        let channels = inner.channels();
        let output_rate = inner.sample_rate();
        let total_duration = inner.total_duration();
        Self { inner, samples_played: counter, channels, output_rate, total_duration }
    }

    /// Convert a raw sample count into milliseconds.
    pub fn ms_from_samples(samples: u64, channels: u16, rate: u32) -> u64 {
        let denom = channels as u64 * rate as u64;
        if denom == 0 { return 0; }
        samples * 1_000 / denom
    }
}

impl Iterator for PositionSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let s = self.inner.next()?;
        self.samples_played.fetch_add(1, Ordering::Relaxed);
        Some(s)
    }
}

impl Source for PositionSource {
    fn current_frame_len(&self) -> Option<usize> { self.inner.current_frame_len() }
    fn channels(&self) -> u16 { self.channels }
    fn sample_rate(&self) -> u32 { self.output_rate }
    fn total_duration(&self) -> Option<Duration> { self.total_duration }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)?;
        self.samples_played.store(0, Ordering::Relaxed);
        Ok(())
    }
}
