-- When a track first entered the library. `scanned_at` is reset whenever a
-- changed file is rescanned (e.g. after editing its tags), so it can't answer
-- "recently added". Same format as scanned_at ('YYYY-MM-DD HH:MM:SS', UTC);
-- set by the scanner on INSERT only (ALTER can't default to CURRENT_TIMESTAMP).
ALTER TABLE tracks ADD COLUMN added_at DATETIME;
UPDATE tracks SET added_at = scanned_at;
CREATE INDEX IF NOT EXISTS idx_tracks_added_at ON tracks(added_at);
