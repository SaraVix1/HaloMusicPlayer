CREATE TABLE IF NOT EXISTS smart_playlists (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    name           TEXT    NOT NULL,
    match_mode     TEXT    NOT NULL DEFAULT 'all',
    sort_field     TEXT    NOT NULL DEFAULT 'title',
    sort_direction TEXT    NOT NULL DEFAULT 'asc',
    limit_count    INTEGER,
    created_at     INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at     INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE TABLE IF NOT EXISTS smart_playlist_rules (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES smart_playlists(id) ON DELETE CASCADE,
    field       TEXT    NOT NULL,
    operator    TEXT    NOT NULL,
    value       TEXT    NOT NULL,
    position    INTEGER NOT NULL DEFAULT 0
);

-- Built-in playlists (INSERT OR IGNORE so re-running the migration is safe)
INSERT OR IGNORE INTO smart_playlists (id, name, match_mode, sort_field, sort_direction, limit_count)
VALUES
    (1, 'Recently Added',  'all', 'date_added',     'desc', 100),
    (2, 'Most Played',     'all', 'play_count',      'desc',  50),
    (3, 'Top Rated',       'all', 'rating',          'desc', NULL),
    (4, 'Never Played',    'all', 'date_added',      'desc', NULL),
    (5, 'Recently Played', 'all', 'last_played_at',  'desc',  50);

INSERT OR IGNORE INTO smart_playlist_rules (playlist_id, field, operator, value, position)
VALUES
    (1, 'date_added',     'in_last_days', '30', 0),
    (2, 'play_count',     'gte',          '1',  0),
    (3, 'rating',         'gte',          '4',  0),
    (4, 'play_count',     'eq',           '0',  0),
    (5, 'last_played_at', 'in_last_days', '7',  0);
