use rusqlite::Connection;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Increment play_count and set last_played_at. Called when a track ends
/// naturally or when the user skips past the scrobble threshold.
pub fn record_play(conn: &Connection, track_id: i64) {
    let now = now_unix();
    let _ = conn.execute(
        "UPDATE tracks SET play_count = play_count + 1, last_played_at = ?1 WHERE id = ?2",
        rusqlite::params![now, track_id],
    );
}

/// Increment skip_count. Called when the user skips before the play threshold.
pub fn record_skip(conn: &Connection, track_id: i64) {
    let _ = conn.execute(
        "UPDATE tracks SET skip_count = skip_count + 1 WHERE id = ?1",
        rusqlite::params![track_id],
    );
}


#[tauri::command]
pub fn set_rating(app: AppHandle, track_id: i64, rating: i64) -> Result<(), String> {
    if !(0..=5).contains(&rating) {
        return Err("rating must be 0–5".into());
    }
    let db = app.state::<Mutex<Connection>>();
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE tracks SET rating = ?1 WHERE id = ?2",
        rusqlite::params![rating, track_id],
    )
    .map_err(|e| e.to_string())?;
    // Notify all windows so now-playing rating updates immediately.
    let _ = app.emit("rating-changed", (track_id, rating));
    Ok(())
}

#[tauri::command]
pub fn reset_all_stats(db: State<Mutex<Connection>>) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE tracks SET play_count = 0, skip_count = 0, last_played_at = NULL, rating = 0",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
