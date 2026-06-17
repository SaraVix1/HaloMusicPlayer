use rusqlite::{params, Connection};
use serde::Serialize;
use std::sync::Mutex;
use tauri::State;

#[derive(Serialize)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct PlaylistTrack {
    pub playlist_track_id: i64,
    pub position: i64,
    pub track_id: i64,
    pub title: Option<String>,
    pub album_name: Option<String>,
    pub artists: Vec<String>,
    pub duration_ms: Option<i64>,
    pub album_art_path: Option<String>,
    pub file_path: String,
}

#[tauri::command]
pub fn get_playlists(db: State<Mutex<Connection>>) -> Result<Vec<Playlist>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.name, p.created_at, p.updated_at,
                    (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id)
             FROM playlists p
             ORDER BY p.updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                track_count: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_playlist(
    id: i64,
    db: State<Mutex<Connection>>,
) -> Result<(Playlist, Vec<PlaylistTrack>), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let playlist = conn
        .query_row(
            "SELECT p.id, p.name, p.created_at, p.updated_at,
                    (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id)
             FROM playlists p WHERE p.id = ?1",
            [id],
            |row| {
                Ok(Playlist {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    track_count: row.get(4)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT pt.id, pt.position, t.id, t.title, t.album_name, t.duration_ms,
                    t.album_art_path, t.file_path,
                    (SELECT GROUP_CONCAT(name, '||') FROM (
                     SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                     WHERE ta.track_id = t.id ORDER BY a.name COLLATE NOCASE))
             FROM playlist_tracks pt
             JOIN tracks t ON t.id = pt.track_id
             WHERE pt.playlist_id = ?1
             ORDER BY pt.position ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([id], |row| {
            let artists_csv: Option<String> = row.get(8)?;
            Ok(PlaylistTrack {
                playlist_track_id: row.get(0)?,
                position: row.get(1)?,
                track_id: row.get(2)?,
                title: row.get(3)?,
                album_name: row.get(4)?,
                duration_ms: row.get(5)?,
                album_art_path: row.get(6)?,
                file_path: row.get(7)?,
                artists: artists_csv
                    .map(|s| s.split("||").map(String::from).collect())
                    .unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;
    let tracks = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok((playlist, tracks))
}

#[tauri::command]
pub fn create_playlist(name: String, db: State<Mutex<Connection>>) -> Result<Playlist, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("playlist name cannot be empty".into());
    }
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO playlists (name) VALUES (?1)", [trimmed])
        .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT id, name, created_at, updated_at FROM playlists WHERE id = ?1",
        [id],
        |row| {
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                track_count: 0,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_playlist(
    id: i64,
    name: String,
    db: State<Mutex<Connection>>,
) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("playlist name cannot be empty".into());
    }
    let conn = db.lock().map_err(|e| e.to_string())?;
    let changed = conn
        .execute(
            "UPDATE playlists SET name = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            params![id, trimmed],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("playlist not found".into());
    }
    Ok(())
}

#[tauri::command]
pub fn delete_playlist(id: i64, db: State<Mutex<Connection>>) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM playlists WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn add_to_playlist(
    playlist_id: i64,
    track_ids: Vec<i64>,
    db: State<Mutex<Connection>>,
) -> Result<(), String> {
    if track_ids.is_empty() {
        return Ok(());
    }
    let conn = db.lock().map_err(|e| e.to_string())?;
    let next_pos: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            [playlist_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    for (i, tid) in track_ids.iter().enumerate() {
        conn.execute(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
            params![playlist_id, tid, next_pos + i as i64],
        )
        .map_err(|e| e.to_string())?;
    }
    conn.execute(
        "UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
        [playlist_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn remove_from_playlist(
    playlist_track_id: i64,
    db: State<Mutex<Connection>>,
) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let row: Option<(i64, i64)> = conn
        .query_row(
            "SELECT playlist_id, position FROM playlist_tracks WHERE id = ?1",
            [playlist_track_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();
    let Some((playlist_id, position)) = row else {
        return Ok(());
    };
    conn.execute(
        "DELETE FROM playlist_tracks WHERE id = ?1",
        [playlist_track_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE playlist_tracks SET position = position - 1
         WHERE playlist_id = ?1 AND position > ?2",
        params![playlist_id, position],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
        [playlist_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn reorder_playlist_track(
    playlist_track_id: i64,
    new_position: i64,
    db: State<Mutex<Connection>>,
) -> Result<(), String> {
    let mut conn = db.lock().map_err(|e| e.to_string())?;
    let row: Option<(i64, i64)> = conn
        .query_row(
            "SELECT playlist_id, position FROM playlist_tracks WHERE id = ?1",
            [playlist_track_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();
    let Some((playlist_id, old_position)) = row else {
        return Err("playlist track not found".into());
    };
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?1",
            [playlist_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let target = new_position.clamp(0, count - 1);
    if target == old_position {
        return Ok(());
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if target < old_position {
        tx.execute(
            "UPDATE playlist_tracks SET position = position + 1
             WHERE playlist_id = ?1 AND position >= ?2 AND position < ?3",
            params![playlist_id, target, old_position],
        )
        .map_err(|e| e.to_string())?;
    } else {
        tx.execute(
            "UPDATE playlist_tracks SET position = position - 1
             WHERE playlist_id = ?1 AND position <= ?2 AND position > ?3",
            params![playlist_id, target, old_position],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.execute(
        "UPDATE playlist_tracks SET position = ?2 WHERE id = ?1",
        params![playlist_track_id, target],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
        [playlist_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
