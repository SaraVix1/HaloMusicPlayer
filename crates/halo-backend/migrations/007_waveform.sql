-- Per-track loudness peaks for the waveform seek bar.
-- A compact BLOB of normalized 0..=255 amplitude buckets (computed lazily on
-- first play, see scanner::waveform). NULL until generated.
ALTER TABLE tracks ADD COLUMN waveform_peaks BLOB;
