mod audio;
mod commands;
mod db;
mod scanner;
mod tray;
mod watcher;

use rusqlite::Connection;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, Manager};

use audio::media_controls::MediaControlsHandle;
use commands::player::FullPlayerState;
use halo_core::now_playing::{NowPlayingController, NowPlayingMeta, PlaybackInfo, RemoteCommand};

#[cfg(windows)]
const AUMID: &str = "Halo";

#[cfg(windows)]
fn set_app_user_model_id() {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    // Windows SMTC looks up the app's display name by matching this AUMID against
    // a Start Menu shortcut whose System.AppUserModel.ID property is the same
    // string. Without that shortcut the panel shows "Unknown app", so we install
    // one ourselves (Spotify, Discord, VS Code do the same).
    if let Err(e) = ensure_aumid_shortcut() {
        eprintln!("aumid shortcut install failed: {e}");
    }
    let aumid = HSTRING::from(AUMID);
    let _ = unsafe { SetCurrentProcessExplicitAppUserModelID(&aumid) };
}

#[cfg(windows)]
fn ensure_aumid_shortcut() -> Result<(), String> {
    use windows::core::{Interface, HSTRING, PROPVARIANT};
    use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, COINIT_APARTMENTTHREADED,
        CLSCTX_INPROC_SERVER,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    let appdata = std::env::var("APPDATA").map_err(|e| format!("APPDATA: {e}"))?;
    let shortcut_dir =
        std::path::PathBuf::from(&appdata).join(r"Microsoft\Windows\Start Menu\Programs");
    std::fs::create_dir_all(&shortcut_dir).map_err(|e| format!("create_dir_all: {e}"))?;
    let shortcut_path = shortcut_dir.join(format!("{AUMID}.lnk"));

    let exe_path = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "exe has no parent dir".to_string())?;

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("CoCreateInstance(ShellLink): {e}"))?;
        link.SetPath(&HSTRING::from(exe_path.as_os_str()))
            .map_err(|e| format!("SetPath: {e}"))?;
        link.SetWorkingDirectory(&HSTRING::from(exe_dir.as_os_str()))
            .map_err(|e| format!("SetWorkingDirectory: {e}"))?;

        let store: IPropertyStore = link.cast().map_err(|e| format!("cast IPropertyStore: {e}"))?;
        let value: PROPVARIANT = PROPVARIANT::from(AUMID);
        store
            .SetValue(&PKEY_AppUserModel_ID, &value)
            .map_err(|e| format!("SetValue(AUMID): {e}"))?;
        store.Commit().map_err(|e| format!("Commit: {e}"))?;

        let persist: IPersistFile = link.cast().map_err(|e| format!("cast IPersistFile: {e}"))?;
        persist
            .Save(&HSTRING::from(shortcut_path.as_os_str()), true)
            .map_err(|e| format!("Save: {e}"))?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn set_app_user_model_id() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    set_app_user_model_id();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.set_skip_taskbar(false);
                let _ = main.show();
                let _ = main.unminimize();
                let _ = main.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");
            let conn = db::open(app_data_dir).expect("failed to open database");
            let saved_volume: Option<f32> = conn
                .query_row(
                    "SELECT value FROM app_state WHERE key = 'player.volume'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .ok()
                .and_then(|s| s.parse::<f32>().ok());
            let saved_theme: String = conn
                .query_row(
                    "SELECT value FROM app_state WHERE key = 'ui.theme'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap_or_else(|_| "dark".to_string());
            // Restore EQ settings from DB before creating the player, so the
            // first track's EqSource already has the correct coefficients.
            let eq_state = std::sync::Arc::new(audio::eq::EqState::new());
            commands::eq::restore_eq_state(&conn, &eq_state);
            // If the current output device has a saved profile, apply it on top.
            if let Some(dev) = commands::eq::get_current_device_name() {
                commands::eq::load_device_eq_if_exists(&conn, &dev, &eq_state);
            }
            let lastfm_state = commands::lastfm::LastFmState::load(&conn);
            app.manage(Mutex::new(conn));
            app.manage(lastfm_state);

            let player = audio::PlayerHandle::new(saved_volume, std::sync::Arc::clone(&eq_state))
                .expect("failed to init audio");
            app.manage(player);
            app.manage(commands::player::ShuffleHistory::new());
            app.manage(commands::sleep_timer::SleepTimer::new());

            // Media controls (Windows SMTC, macOS MPRemoteCommandCenter, Linux MPRIS).
            let (remote_tx, remote_rx) = mpsc::channel::<RemoteCommand>();
            let hwnd_ptr: Option<*mut std::ffi::c_void> = {
                #[cfg(windows)]
                {
                    app.get_webview_window("main")
                        .and_then(|w| w.hwnd().ok())
                        .map(|h| h.0 as *mut std::ffi::c_void)
                }
                #[cfg(not(windows))]
                {
                    None
                }
            };
            match audio::media_controls::spawn(hwnd_ptr, remote_tx) {
                Ok(handle) => {
                    app.manage(handle);
                }
                Err(e) => eprintln!("media controls init failed: {e}"),
            }

            // Remote command router — dispatches OS media-key events to player commands.
            let app_for_remote = app.handle().clone();
            std::thread::spawn(move || {
                while let Ok(cmd) = remote_rx.recv() {
                    handle_remote_command(&app_for_remote, cmd);
                }
            });

            if let Err(e) = tray::build(app.handle()) {
                eprintln!("tray setup failed: {e}");
            }

            // On close: either hide to tray or quit, based on ui.close_behavior preference.
            if let Some(main) = app.get_webview_window("main") {
                let main_clone = main.clone();
                let app_handle = app.handle().clone();
                main.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let behavior = match app_handle.try_state::<Mutex<Connection>>() {
                            Some(db) => match db.lock() {
                                Ok(conn) => commands::ui::read_close_behavior(&conn),
                                Err(_) => "minimize".to_string(),
                            },
                            None => "minimize".to_string(),
                        };
                        if behavior == "quit" {
                            app_handle.exit(0);
                        } else {
                            let _ = main_clone.set_skip_taskbar(true);
                            let _ = main_clone.hide();
                        }
                    }
                });
            }

            if let Err(e) = commands::window::build_mini_window(app.handle()) {
                eprintln!("mini window setup failed: {e}");
            }

            // Folder watcher — start watching managed folders if the preference is on.
            app.manage(watcher::init(app.handle()));
            watcher::reconfigure(app.handle());

            // Resume the previous session's track/position if the user enabled it.
            commands::player::resume_on_launch(app.handle());

            // Show main window with the correct background color for the saved theme,
            // avoiding the white flash that occurs when the webview default precedes CSS.
            if let Some(main) = app.get_webview_window("main") {
                let bg = if saved_theme == "light" {
                    tauri::utils::config::Color(255, 255, 255, 255)
                } else {
                    tauri::utils::config::Color(10, 10, 10, 255)
                };
                let _ = main.set_background_color(Some(bg));
                if let Some(icon) = app.default_window_icon() {
                    let _ = main.set_icon(icon.clone());
                }
                let _ = main.show();
            }

            // State ticker — drives sleep timer, drains audio events, syncs SMTC, broadcasts state.
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut last_track_id: Option<i64> = None;
                let mut last_status: Option<audio::PlaybackStatus> = None;
                let mut last_device: String = String::new();
                let mut tick_count: u64 = 0;
                loop {
                    std::thread::sleep(Duration::from_millis(250));
                    commands::sleep_timer::tick(&app_handle);
                    commands::player::handle_audio_events(&app_handle);

                    // Check for output device changes every ~1 s (4 × 250 ms).
                    if tick_count % 4 == 0 {
                        if let Some(dev) = commands::eq::get_current_device_name() {
                            if dev != last_device {
                                // Follow the system default: reopen the audio output on the
                                // new device, resuming the current track in place. Skip the
                                // first detection at startup (last_device empty) — the stream
                                // already opened on the correct default.
                                if !last_device.is_empty() {
                                    if let Some(player) = app_handle.try_state::<audio::PlayerHandle>() {
                                        if let Err(e) = player.rebuild_output() {
                                            eprintln!("output device follow failed: {e}");
                                        }
                                    }
                                }
                                if let Some(db) = app_handle.try_state::<Mutex<Connection>>() {
                                    if let (Ok(conn), Some(player)) = (
                                        db.lock(),
                                        app_handle.try_state::<audio::PlayerHandle>(),
                                    ) {
                                        if let Some(cfg) = commands::eq::load_device_eq_if_exists(
                                            &conn, &dev, &player.eq_state,
                                        ) {
                                            let _ = app_handle.emit("eq-state-changed", cfg);
                                        }
                                    }
                                }
                                last_device = dev;
                            }
                        }
                    }
                    // Persist playback progress (~1 s cadence) for resume-on-launch.
                    if tick_count % 4 == 0 {
                        commands::player::persist_playback_progress(&app_handle);
                    }
                    let state = match commands::player::get_player_state(app_handle.clone()) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    // Detect track change for Last.fm now-playing update.
                    if state.current_track.as_ref().map(|t| t.track_id) != last_track_id {
                        if let Some(ref ct) = state.current_track {
                            commands::lastfm::on_track_start(&app_handle, ct.track_id);
                        }
                    }
                    sync_media_controls(
                        &app_handle,
                        &state,
                        &mut last_track_id,
                        &mut last_status,
                        tick_count,
                    );
                    let _ = app_handle.emit("player-state", state);
                    tick_count = tick_count.wrapping_add(1);
                }
            });

            // Spectrum thread — emits 24-band frequency data at ~30 fps for the visualizer.
            let app_for_spectrum = app.handle().clone();
            std::thread::Builder::new()
                .name("halo-spectrum".into())
                .spawn(move || {
                    loop {
                        std::thread::sleep(Duration::from_millis(33));
                        if let Some(player) = app_for_spectrum.try_state::<audio::PlayerHandle>() {
                            let bands = player.compute_spectrum();
                            let _ = app_for_spectrum.emit("spectrum", bands);
                        }
                    }
                })
                .ok();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::folders::get_folders,
            commands::folders::add_folder,
            commands::folders::remove_folder,
            commands::scan::scan_library,
            commands::scan::clear_cache,
            commands::scan::clear_database,
            commands::scan::get_scan_settings,
            commands::scan::set_scan_settings,
            commands::library::get_tracks,
            commands::library::get_albums,
            commands::library::get_artists,
            commands::library::get_album_artists,
            commands::library::get_composers,
            commands::library::get_genres,
            commands::library::get_years,
            commands::library::get_folder_tracks,
            commands::library::get_recently_played,
            commands::library::get_most_played,
            commands::lastfm::lastfm_get_status,
            commands::lastfm::lastfm_set_credentials,
            commands::lastfm::lastfm_start_auth,
            commands::lastfm::lastfm_complete_auth,
            commands::lastfm::lastfm_logout,
            commands::lastfm::lastfm_love,
            commands::lastfm::lastfm_is_loved,
            commands::player::get_player_state,
            commands::player::play_queue_index,
            commands::player::toggle_play_pause,
            commands::player::stop_playback,
            commands::player::seek_to,
            commands::player::get_waveform,
            commands::player::set_volume,
            commands::player::next_track,
            commands::player::previous_track,
            commands::player::set_shuffle,
            commands::player::set_repeat,
            commands::player::get_queue,
            commands::player::set_queue_and_play,
            commands::player::add_to_queue,
            commands::player::remove_from_queue,
            commands::player::clear_queue,
            commands::player::get_crossfade_ms,
            commands::player::set_crossfade_ms,
            commands::playlists::get_playlists,
            commands::playlists::get_playlist,
            commands::playlists::create_playlist,
            commands::playlists::rename_playlist,
            commands::playlists::delete_playlist,
            commands::playlists::add_to_playlist,
            commands::playlists::remove_from_playlist,
            commands::playlists::reorder_playlist_track,
            commands::search::search_library,
            commands::sleep_timer::set_sleep_timer,
            commands::sleep_timer::set_sleep_timer_end_of_song,
            commands::sleep_timer::cancel_sleep_timer,
            commands::sleep_timer::get_sleep_timer,
            commands::lyrics::get_lyrics,
            commands::lyrics::save_lyrics,
            commands::lyrics::fetch_lyrics_online,
            commands::lyrics::search_lyrics_providers,
            commands::stats::set_rating,
            commands::stats::reset_all_stats,
            commands::metadata_editor::get_track_full_metadata,
            commands::metadata_editor::save_track_metadata,
            commands::metadata_editor::process_art,
            commands::metadata_editor::extract_track_art,
            commands::metadata_editor::fetch_art_from_url,
            commands::metadata_editor::search_cover_art,
            commands::ui::get_theme,
            commands::ui::set_theme,
            commands::ui::get_pref,
            commands::ui::set_pref,
            watcher::set_watch_folders,
            commands::window::open_mini_player,
            commands::window::restore_main_window,
            commands::window::save_mini_position,
            commands::smart_playlists::get_smart_playlists,
            commands::smart_playlists::get_smart_playlist,
            commands::smart_playlists::create_smart_playlist,
            commands::smart_playlists::update_smart_playlist,
            commands::smart_playlists::delete_smart_playlist,
            commands::smart_playlists::set_smart_playlist_rules,
            commands::smart_playlists::get_smart_playlist_tracks,
            commands::eq::get_eq,
            commands::eq::set_eq_band,
            commands::eq::set_eq_bypass,
            commands::eq::set_eq_preset,
            commands::eq::set_eq_stereo,
            commands::eq::set_eq_dynamic,
            commands::eq::list_user_presets,
            commands::eq::save_user_preset,
            commands::eq::load_user_preset,
            commands::eq::delete_user_preset,
            commands::eq::get_current_device,
            commands::eq::save_device_eq_profile,
            commands::eq::delete_device_eq_profile,
            commands::eq::list_device_eq_profiles,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn handle_remote_command(app: &tauri::AppHandle, cmd: RemoteCommand) {
    use commands::player::*;
    let result: Result<(), String> = match cmd {
        RemoteCommand::Play | RemoteCommand::Pause | RemoteCommand::Toggle => {
            toggle_play_pause(app.clone())
        }
        RemoteCommand::Next => next_track(app.clone()),
        RemoteCommand::Previous => previous_track(app.clone()),
        RemoteCommand::Stop => stop_playback(app.clone()),
        RemoteCommand::SetPositionMs(ms) => seek_to(app.clone(), ms),
        RemoteCommand::SeekMs(delta) => {
            let current_pos = if let Some(handle) = app.try_state::<audio::PlayerHandle>() {
                handle.snapshot().position_ms as i64
            } else {
                0
            };
            let next = (current_pos + delta).max(0) as u64;
            seek_to(app.clone(), next)
        }
        RemoteCommand::Raise => {
            tray::show_main_window(app);
            Ok(())
        }
    };
    if let Err(e) = result {
        eprintln!("remote command error: {e}");
    }
}

fn sync_media_controls(
    app: &tauri::AppHandle,
    state: &FullPlayerState,
    last_track_id: &mut Option<i64>,
    last_status: &mut Option<audio::PlaybackStatus>,
    tick_count: u64,
) {
    let Some(controls) = app.try_state::<MediaControlsHandle>() else {
        return;
    };

    let track_changed = state.player.track_id != *last_track_id;
    let status_changed = Some(state.player.status) != *last_status;
    // Refresh position roughly every second (4 ticks at 250 ms) so the SMTC scrubber stays accurate.
    let refresh_position = tick_count % 4 == 0;

    // Push playback state FIRST so SMTC is not in Stopped when metadata arrives —
    // Windows drops metadata pushed while the transport is in the Stopped state.
    if track_changed || status_changed || refresh_position {
        let (is_playing, is_stopped) = match state.player.status {
            audio::PlaybackStatus::Playing => (true, false),
            audio::PlaybackStatus::Paused => (false, false),
            audio::PlaybackStatus::Stopped => (false, true),
        };
        controls.set_playback(PlaybackInfo {
            is_playing,
            is_stopped,
            position_ms: state.player.position_ms,
        });
        *last_status = Some(state.player.status);
    }

    if track_changed {
        if let Some(t) = &state.current_track {
            let title = t.title.clone().or_else(|| {
                std::path::Path::new(&t.file_path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(String::from)
            });
            controls.set_metadata(&NowPlayingMeta {
                title,
                artist: if t.artists.is_empty() { None } else { Some(t.artists.join(", ")) },
                album: t.album_name.clone(),
                cover_path: t.album_art_path.clone(),
                duration_ms: state.player.duration_ms,
            });
            *last_track_id = state.player.track_id;
        } else if state.player.track_id.is_none() {
            controls.set_metadata(&NowPlayingMeta::default());
            *last_track_id = None;
        }
        // If track_id is Some but current_track is None (DB lookup raced),
        // skip updating last_track_id so we retry on the next tick.
    }
}
