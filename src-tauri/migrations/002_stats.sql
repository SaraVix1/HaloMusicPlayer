ALTER TABLE tracks ADD COLUMN rating INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tracks ADD COLUMN play_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tracks ADD COLUMN skip_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tracks ADD COLUMN last_played_at INTEGER;

CREATE INDEX IF NOT EXISTS idx_tracks_rating ON tracks(rating);
CREATE INDEX IF NOT EXISTS idx_tracks_play_count ON tracks(play_count);
