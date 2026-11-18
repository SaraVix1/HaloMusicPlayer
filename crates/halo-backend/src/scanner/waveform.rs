//! Loudness-peak extraction for the waveform seek bar.
//!
//! Decodes an audio file once and reduces it to a small array of normalized
//! amplitude buckets (`0..=255`). The result is tiny (a couple hundred bytes)
//! and cached as a `BLOB` on the `tracks` row, so the cost is paid once per
//! track (lazily, on first play — see `commands::player::get_waveform`).

use rodio::{Decoder, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Number of buckets (bars) in the rendered waveform. 200 is plenty of detail
/// for an ~88px-tall bar while keeping the cached blob at 200 bytes.
pub const DEFAULT_BUCKETS: usize = 200;

/// Decode `path` and return `buckets` normalized loudness values (`0..=255`).
///
/// Channels are folded to mono by taking the per-frame peak. Each bucket holds
/// the **RMS** (average energy) of its window — not the peak — so the bar
/// reflects perceived loud/quiet structure instead of flattening out (over a
/// ~1.5s window almost every peak hits near full scale). The array is then
/// normalized against the loudest bucket so quiet tracks still fill the bar.
pub fn extract_peaks(path: &Path, buckets: usize) -> Result<Vec<u8>, String> {
    let buckets = buckets.max(1);

    let file = File::open(path).map_err(|e| format!("open: {e}"))?;
    let decoder =
        Decoder::new(BufReader::new(file)).map_err(|e| format!("decode: {e}"))?;

    let channels = decoder.channels().max(1) as usize;

    // Fold interleaved samples into per-frame peak amplitudes.
    let mut frames: Vec<f32> = Vec::new();
    let mut frame_peak: f32 = 0.0;
    let mut ch = 0usize;
    for sample in decoder {
        let amp = (sample as f32).abs();
        if amp > frame_peak {
            frame_peak = amp;
        }
        ch += 1;
        if ch == channels {
            frames.push(frame_peak);
            frame_peak = 0.0;
            ch = 0;
        }
    }
    if ch != 0 {
        frames.push(frame_peak);
    }

    if frames.is_empty() {
        return Err("no audio frames decoded".to_string());
    }

    // Reduce frames into `buckets` RMS values: accumulate sum-of-squares per
    // window, then take sqrt(mean). RMS reflects loudness and varies across
    // sections, where per-bucket peak would saturate to a flat line.
    let mut sumsq = vec![0.0f64; buckets];
    let mut counts = vec![0u64; buckets];
    let len = frames.len();
    for (i, &amp) in frames.iter().enumerate() {
        let bucket = (i * buckets) / len; // 0..buckets-1
        sumsq[bucket] += (amp as f64) * (amp as f64);
        counts[bucket] += 1;
    }
    let rms: Vec<f32> = sumsq
        .iter()
        .zip(&counts)
        .map(|(&s, &n)| if n > 0 { (s / n as f64).sqrt() as f32 } else { 0.0 })
        .collect();

    // Normalize against the loudest bucket → 0..=255.
    let max = rms.iter().cloned().fold(0.0f32, f32::max);
    if max <= 0.0 {
        return Ok(vec![0u8; buckets]);
    }
    let out = rms
        .iter()
        .map(|&v| ((v / max) * 255.0).round().clamp(0.0, 255.0) as u8)
        .collect();
    Ok(out)
}
