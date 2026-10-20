CREATE TABLE IF NOT EXISTS lastfm_scrobble_queue (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    title      TEXT    NOT NULL,
    artist     TEXT    NOT NULL,
    album      TEXT,
    timestamp  INTEGER NOT NULL,
    attempts   INTEGER NOT NULL DEFAULT 0
);
