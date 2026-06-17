use crate::scanner;
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub async fn scan_library(
    app: AppHandle,
    override_metadata: bool,
) -> Result<scanner::ScanSummary, String> {
    let (delimiters, extensions) = {
        let state = app.state::<Mutex<Connection>>();
        let conn = state.lock().map_err(|e| e.to_string())?;
        scanner::read_scan_options(&conn).map_err(|e| e.to_string())?
    };

    let options = scanner::ScanOptions {
        override_metadata,
        delimiters,
        extensions,
    };

    let app_clone = app.clone();
    tauri::async_runtime::spawn_blocking(move || scanner::run_scan(app_clone, options))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn clear_cache(app: AppHandle) -> Result<(), String> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("album-art");
    scanner::clear_cache(&cache_dir)?;

    let state = app.state::<Mutex<Connection>>();
    let conn = state.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE tracks SET album_art_path = NULL", [])
        .map_err(|e| e.to_string())?;
    drop(conn);
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[tauri::command]
pub fn clear_database(app: AppHandle) -> Result<(), String> {
    let state = app.state::<Mutex<Connection>>();
    let conn = state.lock().map_err(|e| e.to_string())?;
    scanner::clear_database(&conn)?;
    drop(conn);
    let _ = app.emit("library-changed", ());
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ScanSettings {
    pub delimiters: String,
    pub extensions: String,
}

#[tauri::command]
pub fn get_scan_settings(db: State<Mutex<Connection>>) -> Result<ScanSettings, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let delimiters: String = conn
        .query_row(
            "SELECT value FROM app_state WHERE key = 'scan.delimiters'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| ",;|:&".to_string());
    let extensions: String = conn
        .query_row(
            "SELECT value FROM app_state WHERE key = 'scan.extensions'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "mp3,flac,m4a,aac,ogg,wav,opus,wma,aiff,aif".to_string());
    Ok(ScanSettings {
        delimiters,
        extensions,
    })
}

#[tauri::command]
pub fn set_scan_settings(
    settings: ScanSettings,
    db: State<Mutex<Connection>>,
) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO app_state (key, value) VALUES ('scan.delimiters', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [&settings.delimiters],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO app_state (key, value) VALUES ('scan.extensions', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [&settings.extensions],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
