CREATE TABLE IF NOT EXISTS folders (
    id          INTEGER PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    added_at    DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS tracks (
    id              INTEGER PRIMARY KEY,
    file_path       TEXT NOT NULL UNIQUE,
    title           TEXT,
    album_name      TEXT,
    track_number    INTEGER,
    disc_number     INTEGER,
    duration_ms     INTEGER,
    year            INTEGER,
    bitrate         INTEGER,
    sample_rate     INTEGER,
    file_format     TEXT,
    file_size       INTEGER,
    album_art_path  TEXT,
    folder_id       INTEGER REFERENCES folders(id),
    scanned_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at     DATETIME
);

CREATE TABLE IF NOT EXISTS artists (
    id      INTEGER PRIMARY KEY,
    name    TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS album_artists (
    id      INTEGER PRIMARY KEY,
    name    TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS composers (
    id      INTEGER PRIMARY KEY,
    name    TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS genres (
    id      INTEGER PRIMARY KEY,
    name    TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS track_artists (
    track_id    INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
    artist_id   INTEGER REFERENCES artists(id) ON DELETE CASCADE,
    PRIMARY KEY (track_id, artist_id)
);

CREATE TABLE IF NOT EXISTS track_album_artists (
    track_id        INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
    album_artist_id INTEGER REFERENCES album_artists(id) ON DELETE CASCADE,
    PRIMARY KEY (track_id, album_artist_id)
);

CREATE TABLE IF NOT EXISTS track_composers (
    track_id    INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
    composer_id INTEGER REFERENCES composers(id) ON DELETE CASCADE,
    PRIMARY KEY (track_id, composer_id)
);

CREATE TABLE IF NOT EXISTS track_genres (
    track_id    INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
    genre_id    INTEGER REFERENCES genres(id) ON DELETE CASCADE,
    PRIMARY KEY (track_id, genre_id)
);

CREATE TABLE IF NOT EXISTS playlists (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS playlist_tracks (
    id          INTEGER PRIMARY KEY,
    playlist_id INTEGER REFERENCES playlists(id) ON DELETE CASCADE,
    track_id    INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS queue (
    id          INTEGER PRIMARY KEY,
    track_id    INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS app_state (
    key     TEXT PRIMARY KEY,
    value   TEXT
);

CREATE INDEX IF NOT EXISTS idx_tracks_folder ON tracks(folder_id);
CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album_name);

INSERT OR IGNORE INTO app_state (key, value) VALUES ('scan.delimiters', ',;|:&');
INSERT OR IGNORE INTO app_state (key, value) VALUES ('scan.extensions', 'mp3,flac,m4a,aac,ogg,wav,opus,wma,aiff,aif');
