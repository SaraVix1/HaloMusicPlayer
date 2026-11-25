//! Heap tuning for glibc (Linux). Elsewhere both functions do nothing.
//!
//! glibc gives each thread that allocates its own arena (up to 8 per core)
//! and rarely hands freed memory back to the OS. Halo's short-lived worker
//! threads (waveform, cover decodes, lyrics on every track change) used to
//! leave ~120 MB (software renderer) to ~300 MB (GPU) of freed-but-kept
//! heap after 50 skips. See docs/HAZARDS.md.

/// Cap glibc at 2 arenas. Call first thing in `main`, before any thread
/// starts. An explicit `MALLOC_ARENA_MAX` in the environment wins (A/B
/// measurements).
pub fn limit_arenas() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    if std::env::var_os("MALLOC_ARENA_MAX").is_none() {
        // SAFETY: plain libc call; no other thread exists yet.
        unsafe {
            libc::mallopt(libc::M_ARENA_MAX, 2);
        }
    }
}

/// Return free heap pages to the OS (a few ms). The ticker calls it a few
/// seconds after the track changes, once that burst of work is done.
pub fn trim() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: thread-safe libc call with no pointers.
    unsafe {
        libc::malloc_trim(0);
    }
}
