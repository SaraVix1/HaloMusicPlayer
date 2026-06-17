use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, State};

#[derive(Serialize, Deserialize)]
pub struct Folder {
    pub id: i64,
    pub path: String,
    pub added_at: String,
}

#[tauri::command]
pub fn get_folders(db: State<Mutex<Connection>>) -> Result<Vec<Folder>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, path, added_at FROM folders ORDER BY added_at ASC")
        .map_err(|e| e.to_string())?;
    let folders = stmt
        .query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                path: row.get(1)?,
                added_at: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(folders)
}

#[tauri::command]
pub fn add_folder(
    path: String,
    app: AppHandle,
    db: State<Mutex<Connection>>,
) -> Result<Folder, String> {
    let folder = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.execute("INSERT OR IGNORE INTO folders (path) VALUES (?1)", [&path])
            .map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT id, path, added_at FROM folders WHERE path = ?1",
            [&path],
            |row| {
                Ok(Folder {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    added_at: row.get(2)?,
                })
            },
        )
        .map_err(|e| e.to_string())?
    };
    // Keep the folder watcher in sync if it's active.
    crate::watcher::reconfigure(&app);
    Ok(folder)
}

#[tauri::command]
pub fn remove_folder(id: i64, app: AppHandle, db: State<Mutex<Connection>>) -> Result<(), String> {
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM folders WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
    }
    crate::watcher::reconfigure(&app);
    Ok(())
}
