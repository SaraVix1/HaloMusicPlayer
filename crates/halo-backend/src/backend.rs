//! `Backend` — the process-wide service container that replaces Tauri's
//! `AppHandle` + managed state.
//!
//! Every subsystem (DB, audio player, shuffle order, sleep timer, Last.fm,
//! open-files queue, folder watcher, OS media controls) lives in one
//! `Arc<Inner>`. `Backend` is cheap to clone and is what command functions and
//! background threads receive in place of the old `AppHandle`.
//!
//! `state::<T>()` / `try_state::<T>()` keep the old Tauri lookup shape so the
//! command bodies stay unchanged, but the lookup is static: `T` must implement
//! [`Managed`], which maps it to a concrete field.
//!
//! Cross-thread notifications that used to be Tauri events are
//! [`BackendEvent`]s, sent over an unbounded channel that the UI drains via
//! [`Backend::take_events`].

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use halo_core::now_playing::{NowPlayingController, NowPlayingMeta, PlaybackInfo, RemoteCommand};
use rusqlite::Connection;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::audio::{self, media_controls::MediaControlsHandle, PlayerHandle};
use crate::commands::{
    self, eq::EqConfig, lastfm::LastFmState, player::FullPlayerState, player::ShuffleState,
    sleep_timer::SleepTimer, sleep_timer::SleepTimerInfo,
};
use crate::open_files::{self, OpenFilesState};
use crate::scanner::ScanProgress;
use crate::watcher::{self, FolderWatcher};

/// Bundle identifier. Data/cache directories are derived from it exactly as
/// Tauri did (`<data_dir>/com.saravix.halo`, `<cache_dir>/com.saravix.halo`),
/// so an existing library database and album-art cache are picked up as-is
/// (the DB stores absolute art-cache paths).
pub const APP_IDENTIFIER: &str = "com.saravix.halo";

/// Notifications from backend threads to the UI (formerly Tauri events).
#[derive(Debug, Clone)]
pub enum BackendEvent {
    /// Full player snapshot — emitted by the 250 ms ticker and after every
    /// player command (formerly `player-state`).
    PlayerState(Box<FullPlayerState>),
    /// Library contents changed (scan, metadata save, file import, clear DB).
    LibraryChanged,
    ScanProgress(ScanProgress),
    /// Output device changed and its saved EQ profile was applied.
    EqStateChanged(EqConfig),
    ThemeChanged(String),
    SleepTimer(SleepTimerInfo),
    RatingChanged { track_id: i64, rating: i64 },
    /// OS asked to raise the app (media panel "raise", second launch with no
    /// files) — show and focus the main window.
    RaiseMain,
    /// Files were opened via the OS — show the main window unless the mini
    /// player is the visible surface.
    RevealMain,
}

/// Resolved per-user directories.
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> Result<Self, String> {
        let data = dirs::data_dir().ok_or("cannot resolve data directory")?;
        let cache = dirs::cache_dir().ok_or("cannot resolve cache directory")?;
        Ok(Self {
            data_dir: data.join(APP_IDENTIFIER),
            cache_dir: cache.join(APP_IDENTIFIER),
        })
    }

    /// Tauri-compatible accessor (`app.path().app_cache_dir()`).
    pub fn app_cache_dir(&self) -> Result<PathBuf, String> {
        Ok(self.cache_dir.clone())
    }

    /// Tauri-compatible accessor (`app.path().app_data_dir()`).
    pub fn app_data_dir(&self) -> Result<PathBuf, String> {
        Ok(self.data_dir.clone())
    }
}

pub struct Inner {
    db: Mutex<Connection>,
    player: PlayerHandle,
    shuffle: ShuffleState,
    sleep: SleepTimer,
    lastfm: LastFmState,
    open_files: OpenFilesState,
    watcher: OnceLock<FolderWatcher>,
    media: OnceLock<MediaControlsHandle>,
    paths: AppPaths,
    events: UnboundedSender<BackendEvent>,
    events_rx: Mutex<Option<UnboundedReceiver<BackendEvent>>>,
    initial_theme: String,
}

/// Cheap, cloneable handle to all backend services.
#[derive(Clone)]
pub struct Backend(Arc<Inner>);

/// Types reachable through `Backend::state` / `try_state`.
pub trait Managed: Sized {
    fn get(inner: &Inner) -> Option<&Self>;
}

macro_rules! managed {
    ($t:ty, $field:ident) => {
        impl Managed for $t {
            fn get(inner: &Inner) -> Option<&Self> {
                Some(&inner.$field)
            }
        }
    };
    ($t:ty, $field:ident, once) => {
        impl Managed for $t {
            fn get(inner: &Inner) -> Option<&Self> {
                inner.$field.get()
            }
        }
    };
}

managed!(Mutex<Connection>, db);
managed!(PlayerHandle, player);
managed!(ShuffleState, shuffle);
managed!(SleepTimer, sleep);
managed!(LastFmState, lastfm);
managed!(OpenFilesState, open_files);
managed!(FolderWatcher, watcher, once);
managed!(MediaControlsHandle, media, once);

impl Backend {
    /// Open the database, restore EQ, create the audio player and start the
    /// background threads (state ticker, folder watcher). Does **not** start
    /// OS media controls — on Windows those need the main window's HWND, so
    /// the UI calls [`Backend::start_media_controls`] once the window exists.
    pub fn start() -> Result<Self, String> {
        let paths = AppPaths::resolve()?;
        std::fs::create_dir_all(&paths.data_dir)
            .map_err(|e| format!("failed to create app data dir: {e}"))?;
        let conn = crate::db::open(paths.data_dir.clone())
            .map_err(|e| format!("failed to open database: {e}"))?;
        crate::profile::mark("db open + migrations");

        let saved_volume: Option<f32> = read_app_state(&conn, "player.volume")
            .and_then(|s| s.parse::<f32>().ok());
        let initial_theme =
            read_app_state(&conn, "ui.theme").unwrap_or_else(|| "dark".to_string());

        // Restore EQ settings from DB before creating the player, so the
        // first track's EqSource already has the correct coefficients.
        let eq_state = Arc::new(audio::eq::EqState::new());
        commands::eq::restore_eq_state(&conn, &eq_state);
        crate::profile::mark("eq restored");
        // If the current output device has a saved profile, apply it on top.
        if let Some(dev) = commands::eq::get_current_device_name() {
            commands::eq::load_device_eq_if_exists(&conn, &dev, &eq_state);
        }
        crate::profile::mark("output device name (per-device EQ)");
        let lastfm = LastFmState::load(&conn);
        crate::profile::mark("last.fm state loaded");

        let player = PlayerHandle::new(saved_volume, Arc::clone(&eq_state))
            .map_err(|e| format!("failed to init audio: {e}"))?;
        crate::profile::mark("audio player created");

        let (tx, rx) = unbounded_channel();
        let backend = Backend(Arc::new(Inner {
            db: Mutex::new(conn),
            player,
            shuffle: ShuffleState::default(),
            sleep: SleepTimer::new(),
            lastfm,
            open_files: OpenFilesState::default(),
            watcher: OnceLock::new(),
            media: OnceLock::new(),
            paths,
            events: tx,
            events_rx: Mutex::new(Some(rx)),
            initial_theme,
        }));

        // Folder watcher — start watching managed folders if the preference is on.
        let _ = backend.0.watcher.set(watcher::init(&backend));
        watcher::reconfigure(&backend);
        crate::profile::mark("folder watcher");

        backend.spawn_ticker();
        crate::profile::mark("backend started");
        Ok(backend)
    }

    /// Typed state lookup (Tauri `Manager::state` replacement). Panics only
    /// if `T` is an optional service that hasn't been initialised — use
    /// `try_state` for those.
    pub fn state<T: Managed>(&self) -> &T {
        T::get(&self.0).expect("backend state not initialised")
    }

    pub fn try_state<T: Managed>(&self) -> Option<&T> {
        T::get(&self.0)
    }

    pub fn db(&self) -> &Mutex<Connection> {
        &self.0.db
    }

    pub fn player(&self) -> &PlayerHandle {
        &self.0.player
    }

    pub fn path(&self) -> &AppPaths {
        &self.0.paths
    }

    /// Theme persisted at startup (`dark` / `light` / `system`).
    pub fn initial_theme(&self) -> &str {
        &self.0.initial_theme
    }

    /// Send a notification to the UI. Never call while holding the DB lock if
    /// the UI might synchronously call back into the DB (see HAZARDS.md).
    pub fn emit(&self, event: BackendEvent) {
        let _ = self.0.events.send(event);
    }

    /// Take the event receiver. Returns `None` after the first call.
    pub fn take_events(&self) -> Option<UnboundedReceiver<BackendEvent>> {
        self.0.events_rx.lock().ok()?.take()
    }

    /// Handle files handed over on launch: play them, or resume the previous
    /// session if there are none.
    pub fn handle_launch_files(&self, files: Vec<PathBuf>) {
        if files.is_empty() {
            commands::player::resume_on_launch(self);
        } else {
            open_files::enqueue(self, files);
        }
    }

    /// A second process forwarded its argv (single-instance). Plays the files,
    /// or just raises the window if there were none.
    pub fn handle_forwarded_args(&self, argv: &[String], cwd: &Path) {
        let files = open_files::paths_from_args(argv, cwd);
        if files.is_empty() {
            self.emit(BackendEvent::RaiseMain);
        } else {
            open_files::enqueue(self, files);
        }
    }

    /// Start OS media controls (Windows SMTC, macOS Now Playing, Linux MPRIS)
    /// and the remote-command router. `hwnd` is required on Windows.
    pub fn start_media_controls(&self, hwnd: Option<*mut std::ffi::c_void>) {
        if self.0.media.get().is_some() {
            return;
        }
        let (remote_tx, remote_rx) = mpsc::channel::<RemoteCommand>();
        match audio::media_controls::spawn(hwnd, remote_tx) {
            Ok(handle) => {
                let _ = self.0.media.set(handle);
            }
            Err(e) => {
                eprintln!("media controls init failed: {e}");
                return;
            }
        }
        let backend = self.clone();
        std::thread::Builder::new()
            .name("halo-remote".into())
            .spawn(move || {
                while let Ok(cmd) = remote_rx.recv() {
                    backend.handle_remote_command(cmd);
                }
            })
            .ok();
    }

    fn handle_remote_command(&self, cmd: RemoteCommand) {
        use commands::player::*;
        let result: Result<(), String> = match cmd {
            RemoteCommand::Play | RemoteCommand::Pause | RemoteCommand::Toggle => {
                toggle_play_pause(self.clone())
            }
            RemoteCommand::Next => next_track(self.clone()),
            RemoteCommand::Previous => previous_track(self.clone()),
            RemoteCommand::Stop => stop_playback(self.clone()),
            RemoteCommand::SetPositionMs(ms) => seek_to(self.clone(), ms),
            RemoteCommand::SeekMs(delta) => {
                let current_pos = self.player().snapshot().position_ms as i64;
                let next = (current_pos + delta).max(0) as u64;
                seek_to(self.clone(), next)
            }
            RemoteCommand::Raise => {
                self.emit(BackendEvent::RaiseMain);
                Ok(())
            }
        };
        if let Err(e) = result {
            eprintln!("remote command error: {e}");
        }
    }

    /// State ticker — drives the sleep timer, drains audio events, follows the
    /// OS default output device, persists progress, syncs OS media controls
    /// and broadcasts the player state every 250 ms.
    fn spawn_ticker(&self) {
        let app = self.clone();
        std::thread::Builder::new()
            .name("halo-ticker".into())
            .spawn(move || {
                let mut last_track_id: Option<i64> = None;
                let mut last_status: Option<audio::PlaybackStatus> = None;
                let mut last_device: String = String::new();
                let mut tick_count: u64 = 0;
                let mut last_snapshot = String::new();
                loop {
                    std::thread::sleep(Duration::from_millis(250));
                    commands::sleep_timer::tick(&app);
                    commands::player::handle_audio_events(&app);

                    // Check for output device changes every ~1 s (4 × 250 ms).
                    if tick_count.is_multiple_of(4) {
                        if let Some(dev) = commands::eq::get_current_device_name() {
                            if dev != last_device {
                                // Follow the system default: reopen the audio output on the
                                // new device, resuming the current track in place. Skip the
                                // first detection at startup (last_device empty) — the stream
                                // already opened on the correct default.
                                if !last_device.is_empty() {
                                    if let Err(e) = app.player().rebuild_output() {
                                        eprintln!("output device follow failed: {e}");
                                    }
                                }
                                let cfg = match app.db().lock() {
                                    Ok(conn) => commands::eq::load_device_eq_if_exists(
                                        &conn,
                                        &dev,
                                        &app.player().eq_state,
                                    ),
                                    Err(_) => None,
                                };
                                if let Some(cfg) = cfg {
                                    app.emit(BackendEvent::EqStateChanged(cfg));
                                }
                                last_device = dev;
                            }
                        }
                    }
                    // Persist playback progress (~1 s cadence) for resume-on-launch.
                    if tick_count.is_multiple_of(4) {
                        commands::player::persist_playback_progress(&app);
                    }
                    let state = match commands::player::get_player_state(app.clone()) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    // Detect track change for Last.fm now-playing update.
                    if state.current_track.as_ref().map(|t| t.track_id) != last_track_id {
                        if let Some(ref ct) = state.current_track {
                            commands::lastfm::on_track_start(&app, ct.track_id);
                        }
                    }
                    app.sync_media_controls(
                        &state,
                        &mut last_track_id,
                        &mut last_status,
                        tick_count,
                    );
                    // Only when something changed: an unchanged state (paused,
                    // stopped, idle) used to redraw the whole UI 4×/s.
                    let snapshot = serde_json::to_string(&state).unwrap_or_default();
                    if snapshot != last_snapshot {
                        last_snapshot = snapshot;
                        app.emit(BackendEvent::PlayerState(Box::new(state)));
                    }
                    tick_count = tick_count.wrapping_add(1);
                }
            })
            .expect("failed to spawn ticker thread");
    }

    fn sync_media_controls(
        &self,
        state: &FullPlayerState,
        last_track_id: &mut Option<i64>,
        last_status: &mut Option<audio::PlaybackStatus>,
        tick_count: u64,
    ) {
        let Some(controls) = self.try_state::<MediaControlsHandle>() else {
            // Without media controls, still track the current track so the
            // Last.fm now-playing hook fires once per track change.
            *last_track_id = state.current_track.as_ref().map(|t| t.track_id);
            return;
        };

        let track_changed = state.player.track_id != *last_track_id;
        let status_changed = Some(state.player.status) != *last_status;
        // Refresh position roughly every second (4 ticks at 250 ms) so the SMTC scrubber stays accurate.
        let refresh_position = tick_count.is_multiple_of(4);

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
                    artist: if t.artists.is_empty() {
                        None
                    } else {
                        Some(t.artists.join(", "))
                    },
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
}

fn read_app_state(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM app_state WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    )
    .ok()
}
