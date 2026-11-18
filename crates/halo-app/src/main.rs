// Prevents an additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

fn main() -> iced::Result {
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
