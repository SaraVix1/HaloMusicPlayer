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

/// A rectangle in physical pixels: `(x, y, width, height)`.
pub type Rect = (i32, i32, i32, i32);

/// Keep a `w`×`h` window at `(x, y)` fully inside `area` (all physical
/// pixels). If the window is larger than the area it is pinned to the
/// area's top-left corner.
pub fn clamp_into(x: i32, y: i32, w: i32, h: i32, area: Rect) -> (i32, i32) {
    let (ax, ay, aw, ah) = area;
    let max_x = (ax + aw - w).max(ax);
    let max_y = (ay + ah - h).max(ay);
    (x.clamp(ax, max_x), y.clamp(ay, max_y))
}

/// Work area (screen minus taskbar) of the monitor nearest to the physical
/// point `(x, y)`. `None` where the OS doesn't expose it (non-Windows).
#[cfg(windows)]
pub fn work_area_near(x: i32, y: i32) -> Option<Rect> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: plain Win32 queries; `info` is a properly sized MONITORINFO.
    let ok = unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        GetMonitorInfoW(monitor, &mut info)
    };
    if !ok.as_bool() {
        return None;
    }
    let r = info.rcWork;
    Some((r.left, r.top, r.right - r.left, r.bottom - r.top))
}

#[cfg(not(windows))]
pub fn work_area_near(_x: i32, _y: i32) -> Option<Rect> {
    None
}

/// Where the mini window should open, in physical pixels: the saved
/// position (Tauri and iced builds both store physical pixels), clamped
/// into the work area of the nearest monitor so a position saved on a
/// monitor that's gone — or at another display scale — can't leave the
/// window off-screen. Without a saved position: bottom-right of the
/// primary work area (`margin` from the edges). `None` = no saved position
/// and no work area known; the UI then picks its own default.
pub fn open_position_physical(db: &Mutex<Connection>, w: i32, h: i32, margin: i32) -> Option<(i32, i32)> {
    match read_position(db) {
        Some((x, y)) => Some(match work_area_near(x + w / 2, y + h / 2) {
            Some(area) => clamp_into(x, y, w, h, area),
            None => (x, y),
        }),
        None => {
            let (ax, ay, aw, ah) = work_area_near(0, 0)?;
            Some(clamp_into(ax + aw - w - margin, ay + ah - h - margin, w, h, (ax, ay, aw, ah)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::clamp_into;

    #[test]
    fn clamp_keeps_inside() {
        let area = (0, 0, 1920, 1040);
        assert_eq!(clamp_into(100, 100, 420, 45, area), (100, 100));
        // Off the right / bottom (e.g. a monitor that was unplugged).
        assert_eq!(clamp_into(2448, 1491, 420, 45, area), (1500, 995));
        // Off the left / top.
        assert_eq!(clamp_into(-3000, -20, 420, 45, area), (0, 0));
    }

    #[test]
    fn clamp_secondary_monitor() {
        let area = (1920, 0, 2560, 1400);
        assert_eq!(clamp_into(2000, 500, 420, 45, area), (2000, 500));
        assert_eq!(clamp_into(100, 500, 420, 45, area), (1920, 500));
    }

    #[test]
    fn clamp_window_larger_than_area() {
        assert_eq!(clamp_into(50, 50, 500, 500, (10, 20, 300, 300)), (10, 20));
    }
}
