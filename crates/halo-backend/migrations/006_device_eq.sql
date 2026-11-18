CREATE TABLE IF NOT EXISTS device_eq_profiles (
    device_name TEXT PRIMARY KEY,
    bypass      INTEGER NOT NULL DEFAULT 0,
    bands       TEXT NOT NULL,
    updated_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
