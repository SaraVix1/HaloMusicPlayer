use rusqlite::Connection;
use std::sync::Mutex;
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

const AOT_KEY: &str = "mini.always_on_top";
const POS_X_KEY: &str = "mini.pos.x";
const POS_Y_KEY: &str = "mini.pos.y";

const MINI_W: f64 = 280.0;
const MINI_H: f64 = 30.0;
const TRAY_MARGIN: f64 = 8.0;

fn read_aot(conn: &Connection) -> bool {
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

fn read_mini_position(app: &AppHandle) -> Option<(i32, i32)> {
    let db = app.try_state::<Mutex<Connection>>()?;
    let conn = db.lock().ok()?;
    let x = read_int(&conn, POS_X_KEY)?;
    let y = read_int(&conn, POS_Y_KEY)?;
    Some((x, y))
}

#[cfg(windows)]
fn workarea_physical() -> Option<(i32, i32, i32, i32)> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    let mut rect = RECT::default();
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if ok.is_err() {
        return None;
    }
    Some((
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    ))
}

#[cfg(windows)]
fn default_mini_position(app: &AppHandle) -> Option<(i32, i32)> {
    let monitor = app.primary_monitor().ok().flatten()?;
    let scale = monitor.scale_factor();
    let (ax, ay, aw, ah) = workarea_physical()?;

    let w_phys = (MINI_W * scale) as i32;
    let h_phys = (MINI_H * scale) as i32;
    let margin = (TRAY_MARGIN * scale) as i32;
    let x = ax + aw - w_phys - margin;
    let y = ay + ah - h_phys - margin;
    Some((x, y))
}

#[cfg(target_os = "linux")]
fn default_mini_position(app: &AppHandle) -> Option<(i32, i32)> {
    let monitor = app.primary_monitor().ok().flatten()?;
    let scale = monitor.scale_factor();
    let size = monitor.size(); // physical pixels
    let w_phys = (MINI_W * scale) as i32;
    let h_phys = (MINI_H * scale) as i32;
    let margin_right = (TRAY_MARGIN * scale) as i32;
    // Use a generous bottom margin since we can't query the work area on Linux
    // without platform-specific APIs; 56 logical px clears most taskbars/docks.
    let margin_bottom = (56.0 * scale) as i32;
    let x = size.width as i32 - w_phys - margin_right;
    let y = size.height as i32 - h_phys - margin_bottom;
    Some((x, y))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn default_mini_position(app: &AppHandle) -> Option<(i32, i32)> {
    let _ = app;
    None
}

fn clamp_to_workarea(window: &WebviewWindow, x: i32, y: i32) -> (i32, i32) {
    #[cfg(windows)]
    {
        if let (Some((ax, ay, aw, ah)), Ok(size)) =
            (workarea_physical(), window.outer_size())
        {
            let w = size.width as i32;
            let h = size.height as i32;
            let max_x = (ax + aw - w).max(ax);
            let max_y = (ay + ah - h).max(ay);
            return (x.clamp(ax, max_x), y.clamp(ay, max_y));
        }
    }
    #[cfg(not(windows))]
    {
        let _ = window;
    }
    (x, y)
}

#[cfg(windows)]
fn apply_sharp_corners(window: &WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    };

    let Ok(raw) = window.hwnd() else { return; };
    let hwnd = HWND(raw.0 as *mut core::ffi::c_void);
    let pref: u32 = DWMWCP_DONOTROUND.0 as u32;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

#[cfg(not(windows))]
fn apply_sharp_corners(window: &WebviewWindow) {
    let _ = window;
}

pub fn build_mini_window(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window("mini").is_some() {
        return Ok(());
    }
    let aot = app
        .try_state::<Mutex<Connection>>()
        .and_then(|db| db.lock().ok().map(|c| read_aot(&c)))
        .unwrap_or(true);

    WebviewWindowBuilder::new(app, "mini", WebviewUrl::App("index.html".into()))
        .title("Halo Mini")
        .inner_size(MINI_W, MINI_H)
        .min_inner_size(MINI_W, MINI_H)
        .max_inner_size(MINI_W, MINI_H)
        .resizable(false)
        .decorations(false)
        // transparent(true) is required on Linux: without it, GTK enforces a minimum
        // window height that is larger than 30 px, making the mini player too tall.
        // CSS backgrounds in index.html / mini-player.tsx remain opaque, so this is
        // invisible to the user on all platforms.
        .transparent(true)
        .always_on_top(aot)
        .skip_taskbar(true)
        .visible(false)
        .devtools(true)
        // The initialization script runs before the page loads.
        // It sets the html/body background to transparent so the
        // compositor-imposed minimum window height (often 120–200 px on
        // Wayland) shows as invisible click-through space rather than a
        // solid dark rectangle below the 30 px content strip.
        .initialization_script(&format!(
            r#"window.__HALO_MINI__ = true;
document.documentElement.style.background = 'transparent';
document.documentElement.style.height = '{h}px';
document.documentElement.style.overflow = 'hidden';
document.addEventListener('DOMContentLoaded', function() {{
    document.body.style.background = 'transparent';
    document.body.style.height = '{h}px';
    document.body.style.overflow = 'hidden';
    document.body.style.margin = '0';
}});"#,
            h = MINI_H as u32
        ))
        .build()
        .map_err(|e| e.to_string())?;

    if let Some(window) = app.get_webview_window("mini") {
        apply_sharp_corners(&window);
        // GTK may ignore the inner_size set in the builder for very small windows.
        // Calling set_size after build enforces the constraint explicitly.
        #[cfg(target_os = "linux")]
        let _ = window.set_size(LogicalSize::new(MINI_W, MINI_H));
        let pos = read_mini_position(app).or_else(|| default_mini_position(app));
        match pos {
            Some((x, y)) => {
                let (cx, cy) = clamp_to_workarea(&window, x, y);
                let _ = window.set_position(PhysicalPosition::new(cx, cy));
            }
            None => {
                let _ = window.center();
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn open_mini_player(app: AppHandle, db: State<Mutex<Connection>>) -> Result<(), String> {
    let aot = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        read_aot(&conn)
    };

    if app.get_webview_window("mini").is_none() {
        build_mini_window(&app)?;
    }

    if let Some(window) = app.get_webview_window("mini") {
        window.show().map_err(|e| e.to_string())?;
        window.unminimize().ok();
        window.set_always_on_top(aot).ok();
        window.set_focus().ok();

        // On Linux/Wayland the compositor maps the window asynchronously after
        // show(), and a single re-apply can still race the map event depending
        // on system load. Retry several times over ~1.5s so one of the attempts
        // is guaranteed to land after the window is fully mapped — matching
        // what happens when the user toggles "Always on Top" by hand from the
        // window's title-bar context menu (which always works immediately).
        #[cfg(target_os = "linux")]
        {
            let app2 = app.clone();
            std::thread::spawn(move || {
                // Each value is a gap from the previous iteration, so calls land
                // at roughly 100ms, 250ms, 500ms, 1s and 2s after open.
                for gap_ms in [100, 150, 250, 500, 1000] {
                    std::thread::sleep(std::time::Duration::from_millis(gap_ms));
                    if let Some(w) = app2.get_webview_window("mini") {
                        w.set_always_on_top(aot).ok();
                    } else {
                        break;
                    }
                }
            });
        }
    }

    if let Some(main) = app.get_webview_window("main") {
        let _ = main.set_skip_taskbar(true);
        if let Err(e) = main.hide() {
            eprintln!("failed to hide main window: {e}");
        }
    }
    Ok(())
}

#[tauri::command]
pub fn restore_main_window(app: AppHandle) -> Result<(), String> {
    if let Some(mini) = app.get_webview_window("mini") {
        let _ = mini.hide();
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.set_skip_taskbar(false);
        main.show().map_err(|e| e.to_string())?;
        let _ = main.unminimize();
        main.set_focus().ok();
    }
    Ok(())
}

#[tauri::command]
pub fn save_mini_position(
    x: i32,
    y: i32,
    app: AppHandle,
    db: State<Mutex<Connection>>,
) -> Result<(), String> {
    let (cx, cy) = if let Some(window) = app.get_webview_window("mini") {
        let (cx, cy) = clamp_to_workarea(&window, x, y);
        if cx != x || cy != y {
            let _ = window.set_position(PhysicalPosition::new(cx, cy));
        }
        (cx, cy)
    } else {
        (x, y)
    };

    let conn = db.lock().map_err(|e| e.to_string())?;
    let x_str = cx.to_string();
    let y_str = cy.to_string();
    conn.execute(
        "INSERT INTO app_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [POS_X_KEY, &x_str],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO app_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [POS_Y_KEY, &y_str],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
