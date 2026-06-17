/// Metadata to push to the OS "now playing" panel (SMTC, MPRIS, MediaSession, MPNowPlaying).
#[derive(Clone, Debug, Default)]
pub struct NowPlayingMeta {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Filesystem path (desktop) or content URI (Android) for album art.
    pub cover_path: Option<String>,
    pub duration_ms: Option<u64>,
}

/// Snapshot of playback state to push to the OS media panel.
#[derive(Clone, Copy, Debug)]
pub struct PlaybackInfo {
    pub is_playing: bool,
    pub is_stopped: bool,
    pub position_ms: u64,
}

/// Command received from the OS media panel or hardware media keys.
/// Single vocabulary for all platform adapters; replaces the old `HotkeyEvent`.
#[derive(Debug, Clone)]
pub enum RemoteCommand {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    /// Seek to an absolute position.
    SetPositionMs(u64),
    /// Seek relative to current position (positive = forward, negative = backward).
    SeekMs(i64),
    /// Bring the app's main window to the foreground (OS media panel click / MPRIS Raise).
    Raise,
}

/// Port for OS "now playing" integration.
///
/// Desktop: `MediaControlsHandle` (souvlaki → Windows SMTC / macOS / Linux MPRIS).
/// Android (Phase D): `MediaSessionCompat` + foreground notification (Kotlin Tauri plugin).
/// iOS (Phase E): `MPNowPlayingInfoCenter` + `MPRemoteCommandCenter` (Swift Tauri plugin).
pub trait NowPlayingController: Send + Sync {
    fn set_metadata(&self, meta: &NowPlayingMeta);
    fn set_playback(&self, info: PlaybackInfo);
}
