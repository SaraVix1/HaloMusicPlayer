use halo_core::now_playing::{
    NowPlayingController, NowPlayingMeta, PlaybackInfo, RemoteCommand,
};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition,
    PlatformConfig,
};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Outbound commands (app → souvlaki thread)
// ---------------------------------------------------------------------------

enum Cmd {
    SetMetadata {
        title: Option<String>,
        artist: Option<String>,
        album: Option<String>,
        /// Raw filesystem path. Windows SMTC cannot load `file://` URIs via
        /// `RandomAccessStreamReference::CreateFromUri`, so we handle the
        /// thumbnail ourselves via `StorageFile` (see `set_windows_thumbnail`).
        cover_path: Option<String>,
        duration_ms: Option<u64>,
    },
    SetPlayback {
        playing: bool,
        stopped: bool,
        position_ms: u64,
    },
    #[allow(dead_code)]
    Shutdown,
}

// ---------------------------------------------------------------------------
// Handle exposed to the rest of the app
// ---------------------------------------------------------------------------

pub struct MediaControlsHandle {
    tx: Mutex<Sender<Cmd>>,
}

impl MediaControlsHandle {
    fn send_cmd(&self, cmd: Cmd) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(cmd);
        }
    }
}

impl NowPlayingController for MediaControlsHandle {
    fn set_metadata(&self, meta: &NowPlayingMeta) {
        self.send_cmd(Cmd::SetMetadata {
            title: meta.title.clone(),
            artist: meta.artist.clone(),
            album: meta.album.clone(),
            cover_path: meta.cover_path.clone(),
            duration_ms: meta.duration_ms,
        });
    }

    fn set_playback(&self, info: PlaybackInfo) {
        self.send_cmd(Cmd::SetPlayback {
            playing: info.is_playing,
            stopped: info.is_stopped,
            position_ms: info.position_ms,
        });
    }
}

// ---------------------------------------------------------------------------
// Spawn the souvlaki worker thread
// ---------------------------------------------------------------------------

/// Start the media-controls thread and return a handle to push updates into it.
/// `remote_tx` receives `RemoteCommand` values whenever the OS panel or hardware
/// keys issue a command (play/pause/next/seek/…).
pub fn spawn(
    hwnd: Option<*mut std::ffi::c_void>,
    remote_tx: Sender<RemoteCommand>,
) -> Result<MediaControlsHandle, String> {
    let (tx, rx) = mpsc::channel::<Cmd>();

    // souvlaki's hwnd is `*mut c_void` — !Send. Transport it as a usize.
    let hwnd_addr: usize = hwnd.map(|p| p as usize).unwrap_or(0);

    let (init_tx, init_rx) = mpsc::channel::<Result<(), String>>();

    std::thread::Builder::new()
        .name("halo-media-controls".into())
        .spawn(move || {
            let hwnd_ptr: Option<*mut std::ffi::c_void> = if hwnd_addr == 0 {
                None
            } else {
                Some(hwnd_addr as *mut std::ffi::c_void)
            };
            // Must match the basename of the installed .desktop file (without
            // the .desktop suffix) so GNOME Shell's media widget can resolve
            // the app icon. The deb bundler names it after `productName`
            // ("Halo Music Player.desktop"); our dev-mode .desktop file
            // (~/.local/share/applications/) is named to match.
            let config = PlatformConfig {
                dbus_name: "halo",
                display_name: "Halo Music Player",
                desktop_entry: "Halo Music Player",
                hwnd: hwnd_ptr,
            };
            let mut controls = match MediaControls::new(config) {
                Ok(c) => c,
                Err(e) => {
                    let _ = init_tx.send(Err(format!("MediaControls::new: {e:?}")));
                    return;
                }
            };

            if let Err(e) = controls.attach(move |event: MediaControlEvent| {
                let cmd = map_event(event);
                if let Some(c) = cmd {
                    let _ = remote_tx.send(c);
                }
            }) {
                let _ = init_tx.send(Err(format!("attach: {e:?}")));
                return;
            }

            let _ = controls.set_playback(MediaPlayback::Stopped);
            let _ = init_tx.send(Ok(()));

            loop {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(Cmd::Shutdown) => break,
                    Ok(Cmd::SetMetadata {
                        title,
                        artist,
                        album,
                        cover_path,
                        duration_ms,
                    }) => {
                        eprintln!(
                            "[SMTC] set_metadata title={:?} artist={:?} album={:?} cover={:?} dur={:?}ms",
                            title, artist, album, cover_path, duration_ms
                        );
                        let cover_url = cover_path.as_deref().map(cover_path_to_file_uri);
                        if let Err(e) = controls.set_metadata(MediaMetadata {
                            title: title.as_deref(),
                            artist: artist.as_deref(),
                            album: album.as_deref(),
                            cover_url: cover_url.as_deref(),
                            duration: duration_ms.map(Duration::from_millis),
                        }) {
                            eprintln!("[SMTC] set_metadata error: {e:?}");
                        }
                        #[cfg(windows)]
                        if hwnd_addr != 0 {
                            if let Err(e) = set_windows_thumbnail(hwnd_addr, cover_path.as_deref()) {
                                eprintln!("[SMTC] thumbnail error: {e}");
                            }
                        }
                    }
                    Ok(Cmd::SetPlayback { playing, stopped, position_ms }) => {
                        let progress = Some(MediaPosition(Duration::from_millis(position_ms)));
                        let state = if stopped {
                            MediaPlayback::Stopped
                        } else if playing {
                            MediaPlayback::Playing { progress }
                        } else {
                            MediaPlayback::Paused { progress }
                        };
                        let _ = controls.set_playback(state);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .map_err(|e| e.to_string())?;

    init_rx.recv().map_err(|e| e.to_string())??;
    Ok(MediaControlsHandle { tx: Mutex::new(tx) })
}

/// Build a `file://` URI from a filesystem path for MPRIS's `mpris:artUrl`
/// (Linux). Each path segment is percent-encoded since `mpris:artUrl` is a
/// URI, not a raw path, while `/` separators are preserved.
fn cover_path_to_file_uri(path: &str) -> String {
    let encoded = path
        .split('/')
        .map(urlencoding::encode)
        .collect::<Vec<_>>()
        .join("/");
    if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

fn map_event(event: MediaControlEvent) -> Option<RemoteCommand> {
    match event {
        MediaControlEvent::Play => Some(RemoteCommand::Play),
        MediaControlEvent::Pause => Some(RemoteCommand::Pause),
        MediaControlEvent::Toggle => Some(RemoteCommand::Toggle),
        MediaControlEvent::Next => Some(RemoteCommand::Next),
        MediaControlEvent::Previous => Some(RemoteCommand::Previous),
        MediaControlEvent::Stop => Some(RemoteCommand::Stop),
        MediaControlEvent::Seek(dir) => {
            let delta: i64 = match dir {
                souvlaki::SeekDirection::Forward => 5_000,
                souvlaki::SeekDirection::Backward => -5_000,
            };
            Some(RemoteCommand::SeekMs(delta))
        }
        MediaControlEvent::SeekBy(dir, d) => {
            let ms = d.as_millis() as i64;
            let delta = match dir {
                souvlaki::SeekDirection::Forward => ms,
                souvlaki::SeekDirection::Backward => -ms,
            };
            Some(RemoteCommand::SeekMs(delta))
        }
        MediaControlEvent::SetPosition(MediaPosition(d)) => {
            Some(RemoteCommand::SetPositionMs(d.as_millis() as u64))
        }
        MediaControlEvent::Raise => Some(RemoteCommand::Raise),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Windows-specific album art thumbnail
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn set_windows_thumbnail(hwnd_addr: usize, cover_path: Option<&str>) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Media::SystemMediaTransportControls;
    use windows::Storage::Streams::RandomAccessStreamReference;
    use windows::Storage::StorageFile;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;

    let hwnd = HWND(hwnd_addr as *mut _);
    let interop: ISystemMediaTransportControlsInterop = windows::core::factory::<
        SystemMediaTransportControls,
        ISystemMediaTransportControlsInterop,
    >()
    .map_err(|e| format!("interop factory: {e}"))?;
    let smtc: SystemMediaTransportControls =
        unsafe { interop.GetForWindow(hwnd) }.map_err(|e| format!("GetForWindow: {e}"))?;
    let updater = smtc.DisplayUpdater().map_err(|e| format!("DisplayUpdater: {e}"))?;

    if let Some(path) = cover_path {
        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path))
            .map_err(|e| format!("GetFileFromPathAsync: {e}"))?
            .get()
            .map_err(|e| format!("GetFileFromPathAsync await: {e}"))?;
        let stream = RandomAccessStreamReference::CreateFromFile(&file)
            .map_err(|e| format!("CreateFromFile: {e}"))?;
        updater.SetThumbnail(&stream).map_err(|e| format!("SetThumbnail: {e}"))?;
    }
    updater.Update().map_err(|e| format!("Update: {e}"))?;
    Ok(())
}
