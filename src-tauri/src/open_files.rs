//! Opening audio files handed to Halo by the OS — double-click / "Open with
//! Halo" in Explorer, Finder, Dolphin, Nautilus, …
//!
//! Paths arrive three ways:
//! - Cold start (Windows/Linux): as launch arguments, read in `setup()`.
//! - Halo already running (Windows/Linux): the second process's argv is
//!   forwarded by `tauri-plugin-single-instance`.
//! - macOS: as `RunEvent::Opened` URLs (never argv), cold or warm.
//!
//! All three feed `enqueue`, which coalesces paths for `COALESCE_MS` before
//! acting: Windows Explorer launches one process *per file* for a
//! multi-selection, so without this only the last file would end up playing.
//! The batch is resolved to library tracks (importing files that aren't in the
//! library yet) and replaces the queue.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Formats Halo can decode (the rodio/symphonia features in Cargo.toml).
/// Keep in sync with `bundle.fileAssociations` in tauri.conf.json.
pub const PLAYABLE_EXTENSIONS: &[&str] =
    &["mp3", "flac", "m4a", "aac", "ogg", "oga", "wav", "aiff", "aif"];

/// How long to wait for more files before opening a batch.
const COALESCE_MS: u64 = 300;

/// Paths waiting for the current coalescing window to close.
#[derive(Default)]
pub struct OpenFilesState(Mutex<Vec<PathBuf>>);

pub fn is_playable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| PLAYABLE_EXTENSIONS.iter().any(|p| p.eq_ignore_ascii_case(e)))
}

/// Extract playable file paths from a process's argv. Skips the program path
/// (argv[0]) and `-`/`--` flags, decodes `file://` URIs (some file managers
/// pass `%U`-style URIs), resolves relative paths against `cwd`, and keeps
/// only playable extensions. Pure — existence is checked by `enqueue`.
pub fn paths_from_args(argv: &[String], cwd: &Path) -> Vec<PathBuf> {
    argv.iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .filter_map(|a| arg_to_path(a))
        .map(|p| if p.is_absolute() { p } else { cwd.join(p) })
        .filter(|p| is_playable(p))
        .collect()
}

fn arg_to_path(arg: &str) -> Option<PathBuf> {
    let Some(rest) = arg.strip_prefix("file://") else {
        return Some(PathBuf::from(arg));
    };
    // file:///abs/path or file://localhost/abs/path; other hosts aren't local.
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let decoded = urlencoding::decode(rest).ok()?.into_owned();
    // file:///C:/Music/x.mp3 → C:/Music/x.mp3 on Windows.
    if cfg!(windows) {
        let b = decoded.as_bytes();
        if b.len() > 2 && b[2] == b':' && b[1].is_ascii_alphabetic() {
            return Some(PathBuf::from(&decoded[1..]));
        }
    }
    Some(PathBuf::from(decoded))
}

/// Queue files to be opened. Non-existent / non-playable paths are dropped;
/// the first path of a new batch starts the coalescing timer.
pub fn enqueue(app: &AppHandle, paths: Vec<PathBuf>) {
    let paths: Vec<PathBuf> = paths
        .into_iter()
        .filter(|p| is_playable(p) && p.is_file())
        .collect();
    if paths.is_empty() {
        return;
    }
    let state = app.state::<OpenFilesState>();
    let start_timer = {
        let mut pending = state.0.lock().unwrap_or_else(|e| e.into_inner());
        let was_empty = pending.is_empty();
        pending.extend(paths);
        was_empty
    };
    if start_timer {
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(COALESCE_MS));
            let batch = std::mem::take(
                &mut *app
                    .state::<OpenFilesState>()
                    .0
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()),
            );
            open_now(&app, batch);
        });
    }
}

fn open_now(app: &AppHandle, mut paths: Vec<PathBuf>) {
    // Same file twice in one batch (e.g. clicked twice) plays once.
    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));

    let ids = crate::scanner::import_paths(app, &paths);
    if !ids.is_empty() {
        let _ = app.emit("library-changed", ());
        if let Err(e) = crate::commands::player::set_queue_and_play(app.clone(), ids, 0) {
            eprintln!("open file: playback failed: {e}");
        }
    }
    reveal(app);
}

/// Bring Halo forward: keep the mini player if that's what's showing (the
/// user chose the compact view), otherwise show the main window.
fn reveal(app: &AppHandle) {
    let mini_visible = app
        .get_webview_window("mini")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if !mini_visible {
        crate::tray::show_main_window(app);
    }
}

// Paths in these tests are Unix-style; Windows path handling is exercised by
// the Windows CI job compiling this module.
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn skips_program_path_and_flags() {
        let cwd = Path::new("/home/u");
        let got = paths_from_args(&args(&["/usr/bin/halo", "--verbose", "-x", "/m/a.mp3"]), cwd);
        assert_eq!(got, vec![PathBuf::from("/m/a.mp3")]);
        assert!(paths_from_args(&args(&["/usr/bin/halo"]), cwd).is_empty());
    }

    #[test]
    fn keeps_only_playable_extensions_case_insensitive() {
        let cwd = Path::new("/");
        let got = paths_from_args(
            &args(&["halo", "/m/a.FLAC", "/m/b.txt", "/m/c.Mp3", "/m/d", "/m/e.opus"]),
            cwd,
        );
        assert_eq!(got, vec![PathBuf::from("/m/a.FLAC"), PathBuf::from("/m/c.Mp3")]);
    }

    #[test]
    fn resolves_relative_paths_against_cwd() {
        let got = paths_from_args(&args(&["halo", "Songs/a.mp3"]), Path::new("/home/u/Music"));
        assert_eq!(got, vec![PathBuf::from("/home/u/Music/Songs/a.mp3")]);
    }

    #[test]
    fn decodes_file_uris() {
        let cwd = Path::new("/");
        let got = paths_from_args(
            &args(&[
                "halo",
                "file:///home/u/My%20Music/Adi-Alaye.mp3",
                "file://localhost/home/u/b.flac",
                "file://otherhost/share/c.mp3",
            ]),
            cwd,
        );
        assert_eq!(
            got,
            vec![
                PathBuf::from("/home/u/My Music/Adi-Alaye.mp3"),
                PathBuf::from("/home/u/b.flac"),
            ]
        );
    }

    #[test]
    fn non_ascii_paths_survive() {
        let got = paths_from_args(
            &args(&["halo", "/m/பாடல்.mp3", "file:///m/%E0%AE%AA%E0%AE%BE.flac"]),
            Path::new("/"),
        );
        assert_eq!(got, vec![PathBuf::from("/m/பாடல்.mp3"), PathBuf::from("/m/பா.flac")]);
    }
}
