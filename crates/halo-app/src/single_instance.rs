//! Single-instance guard (replaces `tauri-plugin-single-instance`).
//!
//! The first process listens on a local socket named after the bundle
//! identifier. Later launches connect, send their argv + cwd as one JSON
//! line and exit; the first process feeds that to
//! `Backend::handle_forwarded_args` (play the files, or raise the window).
//! Windows Explorer launches one process per selected file, so this path is
//! hot — the backend coalesces the forwarded files.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use halo_backend::{Backend, APP_IDENTIFIER};
use interprocess::local_socket::{
    prelude::*, GenericNamespaced, ListenerNonblockingMode, ListenerOptions, Name, Stream,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Forward {
    argv: Vec<String>,
    cwd: PathBuf,
}

fn name() -> std::io::Result<Name<'static>> {
    format!("{APP_IDENTIFIER}.sock").to_ns_name::<GenericNamespaced>()
}

/// Try to hand our argv to an already-running Halo. Returns `true` if one
/// accepted it (this process should exit).
pub fn forward_to_running() -> bool {
    let Ok(name) = name() else { return false };
    let Ok(mut stream) = Stream::connect(name) else {
        return false;
    };
    let msg = Forward {
        argv: std::env::args().collect(),
        cwd: std::env::current_dir().unwrap_or_default(),
    };
    let Ok(mut line) = serde_json::to_string(&msg) else {
        return false;
    };
    line.push('\n');
    stream.write_all(line.as_bytes()).is_ok() && stream.flush().is_ok()
}

/// Become the primary instance: accept forwarded launches on a background
/// thread for the lifetime of the process.
pub fn listen(backend: Backend) {
    let Ok(name) = name() else { return };
    let listener = match ListenerOptions::new()
        .name(name)
        .nonblocking(ListenerNonblockingMode::Neither)
        .reclaim_name(true)
        .create_sync()
    {
        Ok(l) => l,
        Err(e) => {
            eprintln!("single-instance listener failed: {e}");
            return;
        }
    };
    std::thread::Builder::new()
        .name("halo-single-instance".into())
        .spawn(move || {
            for conn in listener.incoming().flatten() {
                let mut line = String::new();
                if BufReader::new(conn).read_line(&mut line).is_ok() {
                    if let Ok(msg) = serde_json::from_str::<Forward>(&line) {
                        backend.handle_forwarded_args(&msg.argv, &msg.cwd);
                    }
                }
            }
        })
        .ok();
}
