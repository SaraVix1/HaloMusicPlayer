use rusqlite::Connection;
use std::sync::Mutex;
use crate::backend::{Backend, BackendEvent};

const THEME_KEY: &str = "ui.theme";

pub fn get_theme(db: &Mutex<Connection>) -> Result<String, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let theme: String = conn
        .query_row(
            "SELECT value FROM app_state WHERE key = ?1",
            [THEME_KEY],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "dark".to_string());
    Ok(theme)
}

pub fn set_theme(theme: String, app: Backend, db: &Mutex<Connection>) -> Result<(), String> {
    let valid = matches!(theme.as_str(), "light" | "dark" | "system");
    if !valid {
        return Err("invalid theme".into());
    }
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO app_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [THEME_KEY, &theme],
    )
    .map_err(|e| e.to_string())?;
    drop(conn);
    // Broadcast to all webviews (including mini player) so they apply immediately.
    app.emit(BackendEvent::ThemeChanged(theme));
    Ok(())
}

/// Generic key/value preference store backed by app_state.
/// Returns None if the key has never been set.
pub fn get_pref(key: String, db: &Mutex<Connection>) -> Result<Option<String>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    match conn.query_row(
        "SELECT value FROM app_state WHERE key = ?1",
        [&key],
        |row| row.get::<_, String>(0),
    ) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn set_pref(key: String, value: String, db: &Mutex<Connection>) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO app_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [&key, &value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Reads the close-behavior preference directly from a live connection.
/// Used by the UI close-request handler.
pub fn read_close_behavior(conn: &Connection) -> String {
    conn.query_row(
        "SELECT value FROM app_state WHERE key = 'ui.close_behavior'",
        [],
        |row| row.get::<_, String>(0),
    )
    .unwrap_or_else(|_| "minimize".to_string())
}
