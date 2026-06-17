pub mod art;
pub mod metadata;
pub mod walker;
pub mod waveform;

use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize)]
pub struct ScanProgress {
    pub current: usize,
    pub total: usize,
    pub current_file: String,
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
    pub failed: usize,
    pub done: bool,
}

#[derive(Clone, Serialize)]
pub struct ScanSummary {
    pub total: usize,
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
    pub failed: usize,
}

#[derive(Default)]
struct Counts {
    inserted: usize,
    updated: usize,
    skipped: usize,
    failed: usize,
}

pub struct ScanOptions {
    pub override_metadata: bool,
    pub delimiters: String,
    pub extensions: Vec<String>,
}

pub fn read_scan_options(conn: &Connection) -> rusqlite::Result<(String, Vec<String>)> {
    let delimiters: String = conn
        .query_row(
            "SELECT value FROM app_state WHERE key = 'scan.delimiters'",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| ",;|:&".to_string());
    let extensions: String = conn
        .query_row(
            "SELECT value FROM app_state WHERE key = 'scan.extensions'",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "mp3,flac,m4a,aac,ogg,wav,opus,wma,aiff,aif".to_string());
    let exts: Vec<String> = extensions
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    Ok((delimiters, exts))
}

pub fn run_scan(app: AppHandle, options: ScanOptions) -> Result<ScanSummary, String> {
    let state = app.state::<Mutex<Connection>>();
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("album-art");
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

    let folders: Vec<(i64, PathBuf)> = {
        let conn = state.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, path FROM folders")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                Ok((id, PathBuf::from(path)))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    let mut files: Vec<(i64, PathBuf)> = Vec::new();
    for (folder_id, folder_path) in &folders {
        for path in walker::walk(folder_path, &options.extensions) {
            files.push((*folder_id, path));
        }
    }

    let total = files.len();
    let mut counts = Counts::default();

    let _ = app.emit(
        "scan-progress",
        ScanProgress {
            current: 0,
            total,
            current_file: String::new(),
            inserted: 0,
            updated: 0,
            skipped: 0,
            failed: 0,
            done: false,
        },
    );

    for (idx, (folder_id, path)) in files.iter().enumerate() {
        let display = path.display().to_string();
        match process_file(&app, &state, *folder_id, path, &options, &cache_dir) {
            Ok(Outcome::Inserted) => counts.inserted += 1,
            Ok(Outcome::Updated) => counts.updated += 1,
            Ok(Outcome::Skipped) => counts.skipped += 1,
            Err(_) => counts.failed += 1,
        }

        let _ = app.emit(
            "scan-progress",
            ScanProgress {
                current: idx + 1,
                total,
                current_file: display,
                inserted: counts.inserted,
                updated: counts.updated,
                skipped: counts.skipped,
                failed: counts.failed,
                done: false,
            },
        );
    }

    let summary = ScanSummary {
        total,
        inserted: counts.inserted,
        updated: counts.updated,
        skipped: counts.skipped,
        failed: counts.failed,
    };

    let _ = app.emit(
        "scan-progress",
        ScanProgress {
            current: total,
            total,
            current_file: String::new(),
            inserted: counts.inserted,
            updated: counts.updated,
            skipped: counts.skipped,
            failed: counts.failed,
            done: true,
        },
    );

    let _ = app.emit("library-changed", ());

    Ok(summary)
}

enum Outcome {
    Inserted,
    Updated,
    Skipped,
}

fn process_file(
    _app: &AppHandle,
    state: &Mutex<Connection>,
    folder_id: i64,
    path: &Path,
    options: &ScanOptions,
    cache_dir: &Path,
) -> Result<Outcome, String> {
    let path_str = path.to_string_lossy().to_string();

    let fs_meta = std::fs::metadata(path).ok();
    let modified_at_fs: Option<i64> = fs_meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);
    let file_size: Option<i64> = fs_meta.map(|m| m.len() as i64);

    let existing: Option<(i64, Option<i64>)> = {
        let conn = state.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT id, CAST(strftime('%s', modified_at) AS INTEGER) FROM tracks WHERE file_path = ?1",
            [&path_str],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1).ok().flatten())),
        )
        .ok()
    };

    if let Some((_, existing_modified)) = existing {
        if !options.override_metadata {
            let unchanged = match (existing_modified, modified_at_fs) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            };
            if unchanged {
                return Ok(Outcome::Skipped);
            }
        }
    }

    let meta = metadata::extract(path).map_err(|e| e.to_string())?;

    let album_art_path = if let Some(picture) = &meta.album_art {
        art::cache_picture(picture, cache_dir).ok()
    } else {
        None
    };

    let mut conn = state.lock().map_err(|e| e.to_string())?;

    let outcome = {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let is_update = existing.is_some();
        upsert_track(&tx, &path_str, folder_id, &meta, album_art_path.as_deref(), file_size, modified_at_fs, is_update)?;
        let track_id: i64 = tx
            .query_row(
                "SELECT id FROM tracks WHERE file_path = ?1",
                [&path_str],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        link_multi_values(&tx, track_id, &meta, &options.delimiters)?;
        tx.commit().map_err(|e| e.to_string())?;
        if is_update { Outcome::Updated } else { Outcome::Inserted }
    };

    Ok(outcome)
}

#[allow(clippy::too_many_arguments)]
fn upsert_track(
    conn: &Connection,
    path: &str,
    folder_id: i64,
    meta: &metadata::TrackMetadata,
    album_art_path: Option<&str>,
    file_size: Option<i64>,
    modified_at: Option<i64>,
    is_update: bool,
) -> Result<(), String> {
    if is_update {
        conn.execute(
            "UPDATE tracks SET
                title = ?2, album_name = ?3, track_number = ?4, disc_number = ?5,
                duration_ms = ?6, year = ?7, bitrate = ?8, sample_rate = ?9,
                file_format = ?10, file_size = ?11, album_art_path = COALESCE(?12, album_art_path),
                folder_id = ?13, modified_at = datetime(?14, 'unixepoch'),
                scanned_at = CURRENT_TIMESTAMP,
                lyrics = COALESCE(?15, lyrics)
            WHERE file_path = ?1",
            params![
                path,
                meta.title,
                meta.album_name,
                meta.track_number,
                meta.disc_number,
                meta.duration_ms,
                meta.year,
                meta.bitrate,
                meta.sample_rate,
                meta.file_format,
                file_size,
                album_art_path,
                folder_id,
                modified_at,
                meta.lyrics,
            ],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO tracks (
                file_path, title, album_name, track_number, disc_number,
                duration_ms, year, bitrate, sample_rate, file_format,
                file_size, album_art_path, folder_id, modified_at, lyrics
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, datetime(?14, 'unixepoch'), ?15)",
            params![
                path,
                meta.title,
                meta.album_name,
                meta.track_number,
                meta.disc_number,
                meta.duration_ms,
                meta.year,
                meta.bitrate,
                meta.sample_rate,
                meta.file_format,
                file_size,
                album_art_path,
                folder_id,
                modified_at,
                meta.lyrics,
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn link_multi_values(
    conn: &Connection,
    track_id: i64,
    meta: &metadata::TrackMetadata,
    delimiters: &str,
) -> Result<(), String> {
    conn.execute("DELETE FROM track_artists WHERE track_id = ?1", [track_id])
        .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM track_album_artists WHERE track_id = ?1",
        [track_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM track_composers WHERE track_id = ?1",
        [track_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM track_genres WHERE track_id = ?1", [track_id])
        .map_err(|e| e.to_string())?;

    for name in split_multi(&meta.artists, delimiters) {
        let id = upsert_named(conn, "artists", &name)?;
        conn.execute(
            "INSERT OR IGNORE INTO track_artists (track_id, artist_id) VALUES (?1, ?2)",
            params![track_id, id],
        )
        .map_err(|e| e.to_string())?;
    }
    for name in split_multi(&meta.album_artists, delimiters) {
        let id = upsert_named(conn, "album_artists", &name)?;
        conn.execute(
            "INSERT OR IGNORE INTO track_album_artists (track_id, album_artist_id) VALUES (?1, ?2)",
            params![track_id, id],
        )
        .map_err(|e| e.to_string())?;
    }
    for name in split_multi(&meta.composers, delimiters) {
        let id = upsert_named(conn, "composers", &name)?;
        conn.execute(
            "INSERT OR IGNORE INTO track_composers (track_id, composer_id) VALUES (?1, ?2)",
            params![track_id, id],
        )
        .map_err(|e| e.to_string())?;
    }
    for name in split_multi(&meta.genres, delimiters) {
        let id = upsert_named(conn, "genres", &name)?;
        conn.execute(
            "INSERT OR IGNORE INTO track_genres (track_id, genre_id) VALUES (?1, ?2)",
            params![track_id, id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn upsert_named(conn: &Connection, table: &str, name: &str) -> Result<i64, String> {
    conn.execute(
        &format!("INSERT OR IGNORE INTO {} (name) VALUES (?1)", table),
        [name],
    )
    .map_err(|e| e.to_string())?;
    conn.query_row(
        &format!("SELECT id FROM {} WHERE name = ?1", table),
        [name],
        |row| row.get::<_, i64>(0),
    )
    .map_err(|e| e.to_string())
}

pub fn split_multi(values: &[String], delimiters: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let delim_chars: Vec<char> = delimiters.chars().collect();
    for raw in values {
        for part in raw.split(|c| delim_chars.contains(&c)) {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }
            let s = trimmed.to_string();
            if !out.iter().any(|existing| existing.eq_ignore_ascii_case(&s)) {
                out.push(s);
            }
        }
    }
    out
}

pub fn clear_database(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "DELETE FROM track_artists;
         DELETE FROM track_album_artists;
         DELETE FROM track_composers;
         DELETE FROM track_genres;
         DELETE FROM playlist_tracks;
         DELETE FROM queue;
         DELETE FROM tracks;
         DELETE FROM artists;
         DELETE FROM album_artists;
         DELETE FROM composers;
         DELETE FROM genres;
         DELETE FROM playlists;",
    )
    .map_err(|e| e.to_string())
}

pub fn clear_cache(cache_dir: &Path) -> Result<(), String> {
    if cache_dir.exists() {
        std::fs::remove_dir_all(cache_dir).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(cache_dir).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_multi_dedupes_and_trims() {
        let input = vec!["Artist A; Artist B & Artist C".to_string()];
        let result = split_multi(&input, ",;|:&");
        assert_eq!(result, vec!["Artist A", "Artist B", "Artist C"]);
    }

    #[test]
    fn split_multi_case_insensitive_dedupe() {
        let input = vec!["pop; Pop".to_string()];
        let result = split_multi(&input, ",;|:&");
        assert_eq!(result, vec!["pop"]);
    }

    #[test]
    fn split_multi_respects_custom_delims() {
        let input = vec!["a/b/c".to_string()];
        let result = split_multi(&input, "/");
        assert_eq!(result, vec!["a", "b", "c"]);
    }
}
