use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::commands::{player, window as window_cmd};

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let play_pause = MenuItem::with_id(app, "tray_play_pause", "Play / Pause", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "tray_next", "Next", true, None::<&str>)?;
    let prev = MenuItem::with_id(app, "tray_prev", "Previous", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "tray_show", "Show Halo", true, None::<&str>)?;
    let mini = MenuItem::with_id(app, "tray_mini", "Mini player", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray_quit", "Quit", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[&play_pause, &prev, &next, &sep1, &show, &mini, &sep2, &quit],
    )?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?;

    TrayIconBuilder::with_id("halo-tray")
        .icon(icon)
        .tooltip("Halo")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray_play_pause" => {
                let _ = player::toggle_play_pause(app.clone());
            }
            "tray_next" => {
                let _ = player::next_track(app.clone());
            }
            "tray_prev" => {
                let _ = player::previous_track(app.clone());
            }
            "tray_show" => {
                show_main_window(app);
            }
            "tray_mini" => {
                if let Some(db) = app.try_state::<std::sync::Mutex<rusqlite::Connection>>() {
                    let _ = window_cmd::open_mini_player(app.clone(), db);
                }
            }
            "tray_quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(mini) = app.get_webview_window("mini") {
        let _ = mini.hide();
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_skip_taskbar(false);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
