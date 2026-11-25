//! Startup timing for performance work: with `HALO_PROFILE=1`, `mark`
//! prints the milliseconds since `start()` to stderr; otherwise it does
//! nothing (one atomic load).

use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static ON: OnceLock<bool> = OnceLock::new();

/// Call first thing in `main`.
pub fn start() {
    let _ = START.set(Instant::now());
    let _ = ON.set(std::env::var_os("HALO_PROFILE").is_some_and(|v| v == "1"));
}

pub fn enabled() -> bool {
    ON.get().copied().unwrap_or(false)
}

/// `[profile]   123.4 ms  label`
pub fn mark(label: &str) {
    if enabled() {
        let ms = START.get().map_or(0.0, |s| s.elapsed().as_secs_f64() * 1e3);
        eprintln!("[profile] {ms:>8.1} ms  {label}");
    }
}
