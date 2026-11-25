//! Opening audio files handed to Halo by the OS — double-click / "Open with
//! Halo" in Explorer, Finder, Dolphin, Nautilus, …
//!
//! Paths arrive three ways:
//! - Cold start (Windows/Linux): as launch arguments, read in `setup()`.
//! - Halo already running (Windows/Linux): the second process's argv is
//!   forwarded over the single-instance socket (see the app crate's
//!   `single_instance` module) to `Backend::handle_forwarded_args`.
//! - macOS: as "open documents" Apple Events (never argv), cold or warm.
//!
//! All three feed `enqueue`, which coalesces paths for `COALESCE_MS` before
//! acting: Windows Explorer launches one process *per file* for a
//! multi-selection, so without this only the last file would end up playing.
//! The batch is resolved to library tracks (importing files that aren't in the
//! library yet) and replaces the queue.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use crate::backend::{Backend, BackendEvent};

/// Formats Halo can decode (the rodio/symphonia features in Cargo.toml).
/// Keep in sync with the packager `file-associations` in crates/halo-app/Cargo.toml.
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
pub fn enqueue(app: &Backend, paths: Vec<PathBuf>) {
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

fn open_now(app: &Backend, mut paths: Vec<PathBuf>) {
    // Same file twice in one batch (e.g. clicked twice) plays once.
    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));

    let ids = crate::scanner::import_paths(app, &paths);
    if !ids.is_empty() {
        app.emit(BackendEvent::LibraryChanged);
        if let Err(e) = crate::commands::player::set_queue_and_play(app.clone(), ids, 0) {
            eprintln!("open file: playback failed: {e}");
        }
    }
    reveal(app);
}

// ── Drag and drop onto the window ─────────────────────────────────────────────

/// What was dropped, resolved to music files.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dropped {
    /// Every playable file: dropped files, then each dropped folder's files
    /// (recursively, sorted by name), in drop order.
    pub songs: Vec<PathBuf>,
    /// Dropped folders (they become library folders on "Add to library").
    pub folders: Vec<PathBuf>,
    /// Playable files dropped directly (not inside a dropped folder).
    pub files: Vec<PathBuf>,
}

/// Resolve dropped paths. Folders are walked recursively without following
/// folder links (no link loops); non-audio files are skipped.
pub fn expand(paths: &[PathBuf]) -> Dropped {
    let mut out = Dropped::default();
    let mut seen = std::collections::HashSet::new();
    for p in paths {
        if p.is_dir() {
            out.folders.push(p.clone());
            for entry in walkdir::WalkDir::new(p).follow_links(false).sort_by_file_name().into_iter().flatten() {
                let f = entry.path();
                if entry.file_type().is_file() && is_playable(f) && seen.insert(f.to_path_buf()) {
                    out.songs.push(f.to_path_buf());
                }
            }
        } else if p.is_file() && is_playable(p) && seen.insert(p.clone()) {
            out.files.push(p.clone());
            out.songs.push(p.clone());
        }
    }
    out
}

/// "Play now": import (if new) and replace the queue.
pub fn play_dropped(app: &Backend, songs: &[PathBuf]) -> Result<(), String> {
    let ids = crate::scanner::import_paths(app, songs);
    app.emit(BackendEvent::LibraryChanged);
    if ids.is_empty() {
        return Err("None of the dropped files could be read.".into());
    }
    crate::commands::player::set_queue_and_play(app.clone(), ids, 0)
}

/// "Add to queue": import (if new) and append.
pub fn queue_dropped(app: &Backend, songs: &[PathBuf]) -> Result<(), String> {
    let ids = crate::scanner::import_paths(app, songs);
    app.emit(BackendEvent::LibraryChanged);
    crate::commands::player::add_tracks_to_queue(app.clone(), ids)
}

/// "Add to library": dropped folders become library folders and are
/// scanned (as in Settings → Library); loose files are imported.
pub fn add_dropped_to_library(app: &Backend, d: &Dropped) -> Result<(), String> {
    for f in &d.folders {
        crate::commands::folders::add_folder(f.to_string_lossy().into_owned(), app.clone(), app.db())?;
    }
    if !d.files.is_empty() {
        crate::scanner::import_paths(app, &d.files);
    }
    if !d.folders.is_empty() {
        crate::commands::scan::scan_library(app.clone(), false)?;
    }
    app.emit(BackendEvent::LibraryChanged);
    Ok(())
}

/// Bring Halo forward: the UI keeps the mini player if that's what's showing
/// (the user chose the compact view), otherwise shows the main window.
fn reveal(app: &Backend) {
    app.emit(BackendEvent::RevealMain);
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

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("halo-drop-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"").unwrap();
    }

    #[test]
    fn expand_walks_folders_sorted_and_skips_non_audio() {
        let root = tmp("walk");
        let album = root.join("Album");
        for f in ["02 b.flac", "01 a.mp3", "cover.jpg", "notes.txt", "CD2/01 c.ogg"] {
            touch(&album.join(f));
        }
        let loose = root.join("single.m4a");
        touch(&loose);
        touch(&root.join("readme.txt"));
        let d = expand(&[loose.clone(), album.clone(), root.join("readme.txt"), root.join("missing.mp3")]);
        assert_eq!(
            d.songs,
            vec![loose.clone(), album.join("01 a.mp3"), album.join("02 b.flac"), album.join("CD2/01 c.ogg")]
        );
        assert_eq!(d.folders, vec![album]);
        assert_eq!(d.files, vec![loose]);
    }

    #[test]
    fn expand_dedupes_and_handles_nothing_playable() {
        let root = tmp("dupe");
        let f = root.join("a.mp3");
        touch(&f);
        // The same file dropped twice, and inside a dropped folder too.
        let d = expand(&[f.clone(), f.clone(), root.clone()]);
        assert_eq!(d.songs, vec![f]);
        touch(&root.join("x.jpg"));
        assert!(expand(&[root.join("x.jpg")]).songs.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn expand_does_not_follow_folder_link_loops() {
        let root = tmp("loop");
        touch(&root.join("a.mp3"));
        std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
        let d = expand(&[root.clone()]);
        assert_eq!(d.songs, vec![root.join("a.mp3")]);
    }
}
