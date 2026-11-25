// Prevents an additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accent;
mod app;
mod components;
mod format;
mod icons;
mod pages;
mod route;
mod single_instance;
mod theme;
mod tray;
mod ui;
mod widgets;

use halo_backend::Backend;

const ICON_PNG: &[u8] = include_bytes!("../assets/icons/128x128.png");

/// Window icon (taskbar / alt-tab).
pub fn app_icon() -> Option<iced::window::Icon> {
    let img = image::load_from_memory(ICON_PNG).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    iced::window::icon::from_rgba(img.into_raw(), w, h).ok()
}

/// The session's `WAYLAND_DISPLAY`, saved by [`prefer_x11`] before it is
/// removed, so programs Halo launches (the browser) still get Wayland.
#[cfg(target_os = "linux")]
static WAYLAND_DISPLAY: std::sync::OnceLock<std::ffi::OsString> = std::sync::OnceLock::new();

/// Linux: run under X11 — XWayland on a Wayland session — unless
/// `HALO_WAYLAND=1`. On Wayland winit can't hide a window, keep one above
/// the others or place it, so the mini player would sit under other windows
/// beside a main window that stays visible, and close-to-tray couldn't hide
/// (HAZARDS). winit picks Wayland whenever `WAYLAND_DISPLAY` is set, so
/// remove it. Must run before any thread starts (it changes the environment).
#[cfg(target_os = "linux")]
fn prefer_x11() {
    let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    if !set("DISPLAY") || std::env::var_os("HALO_WAYLAND").is_some_and(|v| v == "1") {
        return;
    }
    if let Some(w) = std::env::var_os("WAYLAND_DISPLAY") {
        let _ = WAYLAND_DISPLAY.set(w);
    }
    std::env::remove_var("WAYLAND_DISPLAY");
    std::env::remove_var("WAYLAND_SOCKET");
}

/// Open a URL in the default browser. On Linux the browser gets the
/// session's `WAYLAND_DISPLAY` back (see [`prefer_x11`]).
pub fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    if let Some(w) = WAYLAND_DISPLAY.get() {
        let mut last = std::io::Error::other("no command to open URLs");
        for mut cmd in open::commands(url) {
            match cmd.env("WAYLAND_DISPLAY", w).status() {
                Ok(s) if s.success() => return Ok(()),
                Ok(s) => last = std::io::Error::other(format!("{cmd:?} exited with {s}")),
                Err(e) => last = e,
            }
        }
        return Err(last);
    }
    open::that(url)
}

fn main() -> iced::Result {
    #[cfg(target_os = "linux")]
    prefer_x11();
    halo_backend::platform::set_app_user_model_id();

    // A second launch hands its argv (files to open) to the running Halo
    // and exits.
    if single_instance::forward_to_running() {
        return Ok(());
    }

    let backend = match Backend::start() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Halo failed to start: {e}");
            std::process::exit(1);
        }
    };
    single_instance::listen(backend.clone());

    // Launched with audio files (double-click / "Open with Halo"): play
    // those instead of resuming the previous session.
    let launch_files = {
        let argv: Vec<String> = std::env::args().collect();
        let cwd = std::env::current_dir().unwrap_or_default();
        halo_backend::open_files::paths_from_args(&argv, &cwd)
    };
    backend.handle_launch_files(launch_files);

    let mut daemon = iced::daemon(
        move || app::Halo::new(backend.clone()),
        app::Halo::update,
        app::Halo::view,
    )
    .title(app::Halo::title)
    .theme(app::Halo::theme)
    .subscription(app::Halo::subscription)
    .default_font(theme::SANS)
    .antialiasing(true);
    for font in theme::FONT_BYTES {
        daemon = daemon.font(*font);
    }
    daemon.run()
}
