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
    let file = File::open(path).map_err(|e| format!("open: {e}"))?;
    let decoder =
        Decoder::new(BufReader::new(file)).map_err(|e| format!("decode: {e}"))?;

    let channels = decoder.channels().max(1) as usize;

    // Fold interleaved samples into per-frame peak amplitudes, streamed into
    // block sums: the whole song is never held in memory (a 5-minute track
    // used to be a ~50 MB `Vec<f32>`, per track change).
    let mut blocks = Blocks::default();
    let mut frame_peak: f32 = 0.0;
    let mut ch = 0usize;
    for sample in decoder {
        let amp = (sample as f32).abs();
        if amp > frame_peak {
            frame_peak = amp;
        }
        ch += 1;
        if ch == channels {
            blocks.push(frame_peak);
            frame_peak = 0.0;
            ch = 0;
        }
    }
    if ch != 0 {
        blocks.push(frame_peak);
    }
    blocks.finish(buckets)
}

/// Most block sums kept: when full, neighbours merge and blocks double in
/// size. 512 KB at most; short clips stay exact (one frame per block) and
/// every bucket of a long song spans 160+ blocks.
const MAX_BLOCKS: usize = 1 << 16;

/// Sum of squared frame peaks per block of `block` frames.
struct Blocks {
    sums: Vec<f64>,
    block: usize,
    cur: f64,
    n: usize,
    frames: usize,
}

impl Default for Blocks {
    fn default() -> Self {
        Self { sums: Vec::new(), block: 1, cur: 0.0, n: 0, frames: 0 }
    }
}

impl Blocks {
    fn push(&mut self, amp: f32) {
        self.cur += (amp as f64) * (amp as f64);
        self.n += 1;
        self.frames += 1;
        if self.n == self.block {
            self.sums.push(std::mem::take(&mut self.cur));
            self.n = 0;
            if self.sums.len() == MAX_BLOCKS {
                for i in 0..MAX_BLOCKS / 2 {
                    self.sums[i] = self.sums[2 * i] + self.sums[2 * i + 1];
                }
                self.sums.truncate(MAX_BLOCKS / 2);
                self.block *= 2;
            }
        }
    }

    /// Reduce to `buckets` RMS values (frame `i` belongs to bucket
    /// `i * buckets / frames`), normalized against the loudest → `0..=255`.
    /// A block inside one bucket adds its exact sum; one straddling a
    /// boundary is split by frame count.
    fn finish(mut self, buckets: usize) -> Result<Vec<u8>, String> {
        let buckets = buckets.max(1);
        let len = self.frames;
        if len == 0 {
            return Err("no audio frames decoded".to_string());
        }
        if self.n > 0 {
            self.sums.push(self.cur);
        }
        // First frame of bucket b: ceil(b * len / buckets).
        let first = |b: usize| (b * len).div_ceil(buckets);
        let mut sumsq = vec![0.0f64; buckets];
        let mut counts = vec![0u64; buckets];
        for (j, &sum) in self.sums.iter().enumerate() {
            let start = j * self.block;
            let end = (start + self.block).min(len);
            let mut b = start * buckets / len;
            if first(b + 1).min(len) >= end {
                sumsq[b] += sum;
                counts[b] += (end - start) as u64;
                continue;
            }
            let mean = sum / (end - start) as f64;
            let mut s = start;
            while s < end {
                let e = first(b + 1).min(end);
                sumsq[b] += mean * (e - s) as f64;
                counts[b] += (e - s) as u64;
                s = e;
                b += 1;
            }
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
        Ok(rms
            .iter()
            .map(|&v| ((v / max) * 255.0).round().clamp(0.0, 255.0) as u8)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The old whole-song implementation, as the reference.
    fn reference(frames: &[f32], buckets: usize) -> Vec<u8> {
        let mut sumsq = vec![0.0f64; buckets];
        let mut counts = vec![0u64; buckets];
        let len = frames.len();
        for (i, &amp) in frames.iter().enumerate() {
            let b = (i * buckets) / len;
            sumsq[b] += (amp as f64) * (amp as f64);
            counts[b] += 1;
        }
        let rms: Vec<f32> = sumsq
            .iter()
            .zip(&counts)
            .map(|(&s, &n)| if n > 0 { (s / n as f64).sqrt() as f32 } else { 0.0 })
            .collect();
        let max = rms.iter().cloned().fold(0.0f32, f32::max);
        rms.iter().map(|&v| ((v / max) * 255.0).round().clamp(0.0, 255.0) as u8).collect()
    }

    fn streamed(frames: &[f32], buckets: usize) -> Vec<u8> {
        let mut b = Blocks::default();
        frames.iter().for_each(|&f| b.push(f));
        b.finish(buckets).unwrap()
    }

    fn signal(len: usize) -> Vec<f32> {
        // Loud / quiet sections plus a fast wobble.
        (0..len)
            .map(|i| {
                let section = if (i / 50_000) % 3 == 0 { 0.2 } else { 0.9 };
                section * (0.5 + 0.5 * ((i as f32) * 0.37).sin().abs())
            })
            .collect()
    }

    #[test]
    fn matches_the_whole_song_version() {
        // Long, short (fewer blocks than buckets) and odd lengths.
        for len in [13_000_000, 3_000_000, 1_234_567, 100_000, 40_000, 255, 7] {
            let f = signal(len);
            let (a, b) = (reference(&f, 200), streamed(&f, 200));
            let worst = a.iter().zip(&b).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
            assert!(worst <= 1, "len {len}: off by {worst}");
        }
    }

    #[test]
    fn silence_and_empty() {
        assert_eq!(streamed(&[0.0; 5000], 10), vec![0; 10]);
        assert!(Blocks::default().finish(10).is_err());
    }
}
