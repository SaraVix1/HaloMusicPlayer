//! Halo backend: library database, scanner, audio engine, playback queue,
//! playlists, EQ, lyrics, Last.fm and OS integrations — everything except the
//! UI. The UI crate (`halo-app`) owns a [`Backend`] and calls the command
//! functions in [`commands`] directly; background threads report back through
//! [`BackendEvent`]s.

pub mod audio;
pub mod backend;
pub mod commands;
pub mod db;
pub mod open_files;
pub mod platform;
pub mod profile;
pub mod scanner;
pub mod watcher;

pub use backend::{AppPaths, Backend, BackendEvent, APP_IDENTIFIER};
