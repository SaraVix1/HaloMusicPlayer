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

/// `[profile] state changed: position_ms, current_track.play_count` — which
/// (nested) JSON fields differ between two player-state snapshots.
pub fn changed_keys(old: &str, new: &str) {
    let (Ok(a), Ok(b)) = (
        serde_json::from_str::<serde_json::Value>(old),
        serde_json::from_str::<serde_json::Value>(new),
    ) else {
        return;
    };
    let mut out = Vec::new();
    diff("", &a, &b, &mut out);
    eprintln!("[profile] state changed: {}", out.join(", "));
}

fn diff(prefix: &str, a: &serde_json::Value, b: &serde_json::Value, out: &mut Vec<String>) {
    use serde_json::Value::Object;
    match (a, b) {
        (Object(x), Object(y)) => {
            for (k, v) in y {
                let p = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                match x.get(k) {
                    Some(w) => diff(&p, w, v, out),
                    None => out.push(p),
                }
            }
        }
        _ if a != b => out.push(prefix.to_string()),
        _ => {}
    }
}
