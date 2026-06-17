-- The first waveform implementation bucketed by peak amplitude, which produced
-- a near-flat bar (every ~1.5s window hits a near-full-scale transient). The
-- extractor now uses RMS per bucket. Clear any peaks cached by the old version
-- so they regenerate lazily on next play.
UPDATE tracks SET waveform_peaks = NULL;
