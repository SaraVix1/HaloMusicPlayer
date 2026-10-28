-- Queue lookups go by position (track_at_index on every Next / Previous /
-- track end). Without an index each lookup sorted the whole queue.
CREATE INDEX IF NOT EXISTS idx_queue_position ON queue(position);
