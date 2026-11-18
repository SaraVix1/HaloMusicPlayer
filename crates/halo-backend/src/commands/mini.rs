//! Mini player preferences persisted in `app_state`: always-on-top and the
//! last window position (physical pixels). The window itself is built by the
//! UI crate; this module only owns the stored values.

use rusqlite::Connection;
use std::sync::Mutex;

const AOT_KEY: &str = "mini.always_on_top";
const POS_X_KEY: &str = "mini.pos.x";
const POS_Y_KEY: &str = "mini.pos.y";

/// Logical size of the mini player window.
pub const MINI_W: f64 = 280.0;
pub const MINI_H: f64 = 30.0;

pub fn read_aot(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT value FROM app_state WHERE key = ?1",
        [AOT_KEY],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .map(|s| s != "false")
    .unwrap_or(true)
}

fn read_int(conn: &Connection, key: &str) -> Option<i32> {
    conn.query_row(
        "SELECT value FROM app_state WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| s.parse::<i32>().ok())
}

pub fn always_on_top(db: &Mutex<Connection>) -> bool {
    db.lock().map(|c| read_aot(&c)).unwrap_or(true)
}

pub fn read_position(db: &Mutex<Connection>) -> Option<(i32, i32)> {
    let conn = db.lock().ok()?;
    let x = read_int(&conn, POS_X_KEY)?;
    let y = read_int(&conn, POS_Y_KEY)?;
    Some((x, y))
}

pub fn save_position(db: &Mutex<Connection>, x: i32, y: i32) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let x_str = x.to_string();
    let y_str = y.to_string();
    for (k, v) in [(POS_X_KEY, &x_str), (POS_Y_KEY, &y_str)] {
        conn.execute(
            "INSERT INTO app_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [k, v.as_str()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
