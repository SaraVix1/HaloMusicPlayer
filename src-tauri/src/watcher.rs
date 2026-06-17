//! Folder watcher — when the "watch folders for changes" preference is on,
//! filesystem changes inside managed folders trigger a debounced library
//! rescan so the library stays in sync without a manual rescan.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager};

use crate::scanner;

const KEY_WATCH: &str = "library.watch_folders";
/// Quiet period after the last change before a rescan fires.
const DEBOUNCE: Duration = Duration::from_secs(2);

/// Managed state: holds the channel the notify callback writes to and the live
/// watcher handle (dropping it stops watching).
pub struct FolderWatcher {
    tx: Sender<()>,
    watcher: Mutex<Option<RecommendedWatcher>>,
}

pub fn is_enabled(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT value FROM app_state WHERE key = ?1",
        [KEY_WATCH],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .map(|s| s == "true")
    .unwrap_or(false)
}

fn managed_folders(conn: &Connection) -> Vec<PathBuf> {
    conn.prepare("SELECT path FROM folders")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(0))
                .map(|rows| rows.flatten().map(PathBuf::from).collect())
        })
        .unwrap_or_default()
}

/// Create the watcher state and spawn the long-lived debounce thread. Call
/// `reconfigure` afterward to start watching if the preference is enabled.
pub fn init(app: &AppHandle) -> FolderWatcher {
    let (tx, rx) = channel::<()>();
    let app_for_thread = app.clone();
    std::thread::Builder::new()
        .name("halo-folder-watch".into())
        .spawn(move || debounce_loop(app_for_thread, rx))
        .ok();
    FolderWatcher {
        tx,
        watcher: Mutex::new(None),
    }
}

/// Coalesces bursts of filesystem events into a single rescan once changes go
/// quiet for `DEBOUNCE`.
fn debounce_loop(app: AppHandle, rx: Receiver<()>) {
    loop {
        // Block until the first change of a burst.
        if rx.recv().is_err() {
            return;
        }
        // Drain until the filesystem has been quiet for DEBOUNCE.
        loop {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(()) => continue,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }

        let options = {
            let state = app.state::<Mutex<Connection>>();
            let Ok(conn) = state.lock() else { continue };
            match scanner::read_scan_options(&conn) {
                Ok((delimiters, extensions)) => Some(scanner::ScanOptions {
                    override_metadata: false,
                    delimiters,
                    extensions,
                }),
                Err(_) => None,
            }
        };
        if let Some(options) = options {
            if scanner::run_scan(app.clone(), options).is_ok() {
                let _ = app.emit("library-changed", ());
            }
        }
    }
}

/// Start, restart, or stop watching based on the current preference and the
/// current set of managed folders. Safe to call repeatedly (on startup, when
/// the toggle changes, and when folders are added/removed).
pub fn reconfigure(app: &AppHandle) {
    let Some(watcher_state) = app.try_state::<FolderWatcher>() else { return };
    let db = app.state::<Mutex<Connection>>();

    let (enabled, folders) = {
        let Ok(conn) = db.lock() else { return };
        (is_enabled(&conn), managed_folders(&conn))
    };

    let Ok(mut guard) = watcher_state.watcher.lock() else { return };

    if !enabled {
        *guard = None; // drop the watcher → stop watching
        return;
    }

    let tx = watcher_state.tx.clone();
    let mut watcher = match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if matches!(
                event.kind,
                EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_)
            ) {
                let _ = tx.send(());
            }
        }
    }) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("folder watcher create failed: {e}");
            return;
        }
    };

    for folder in &folders {
        if let Err(e) = watcher.watch(folder, RecursiveMode::Recursive) {
            eprintln!("watch {folder:?} failed: {e}");
        }
    }
    *guard = Some(watcher);
}

/// IPC: toggle the watch-folders preference and apply it immediately.
#[tauri::command]
pub fn set_watch_folders(app: AppHandle, enabled: bool) -> Result<(), String> {
    {
        let db = app.state::<Mutex<Connection>>();
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO app_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [KEY_WATCH, if enabled { "true" } else { "false" }],
        )
        .map_err(|e| e.to_string())?;
    }
    reconfigure(&app);
    Ok(())
}
