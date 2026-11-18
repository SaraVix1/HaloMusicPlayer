# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

**Halo** is a desktop music player written entirely in Rust: a native UI built with [iced](https://iced.rs) 0.14, SQLite for the library, and rodio/symphonia for audio. See [docs/plan/halo-future-features.md](docs/plan/halo-future-features.md) for the feature roadmap and [docs/ACTIVE_WORK.md](docs/ACTIVE_WORK.md) for what's in flight. Non-obvious traps are collected in [docs/HAZARDS.md](docs/HAZARDS.md).

(Halo was a Tauri v2 + React app until the `iced-port` branch; the UI was ported 1:1 — same shadcn "neutral" look, Geist font, Lucide icons, layout and behaviour.)

## Prerequisites

- **Rust** stable (Windows: MSVC toolchain `stable-x86_64-pc-windows-msvc`; `rust-lld` is configured in [.cargo/config.toml](.cargo/config.toml), see HAZARDS).
- **Linux** build deps: `pkg-config libasound2-dev libdbus-1-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libgtk-3-dev libayatana-appindicator3-dev libxdo-dev` (GTK/appindicator are for the tray icon, dbus for MPRIS).
- No Node.js.

## Common commands

```bash
cargo run -p halo                 # run the app (debug)
cargo build --release -p halo     # release binary → target/release/halo
cargo check --workspace           # check all crates
cargo test --workspace            # all tests (halo-core, halo-backend, halo app)
cargo test -p halo-core           # domain-logic tests only
cargo test <test_name>            # single test
cargo clippy --workspace          # lint

# Installers (cargo install cargo-packager --locked)
cargo packager --release -p halo                      # all formats for this OS
cargo packager --release -p halo --formats deb,appimage

# Set the version by hand (root Cargo.toml [workspace.package] + Cargo.lock)
python3 scripts/bump-version.py <version>
```

**Versioning:** Halo is in beta. `.github/workflows/bump-version.yml` bumps the patch version on every push to `main` (skipped when the commit title contains `[skip bump]`); `release.yml` builds installers with cargo-packager and publishes a "(Beta)" pre-release for a pushed `v*` tag, or — when run manually from `main` — for `v<current version>`, creating that tag.

## Architecture

### Workspace

```
Cargo.toml                      # workspace: crates/* ; [workspace.package] version; dev profiles; souvlaki patch
crates/
├── halo-core/                  # Pure domain logic — no I/O, no UI, no rodio (queue/shuffle/repeat, play &
│                               # scrobble thresholds, NowPlayingController trait, RemoteCommand) + tests
├── halo-backend/               # Everything except the UI (lib crate)
│   ├── migrations/*.sql        # 001–010, wired by hand in db/migrations.rs
│   └── src/
│       ├── backend.rs          # Backend service container (Arc<Inner>): DB, PlayerHandle, ShuffleState,
│       │                       # SleepTimer, LastFmState, OpenFilesState, FolderWatcher, media controls,
│       │                       # AppPaths; typed state::<T>() lookups; BackendEvent channel; 250ms ticker;
│       │                       # start_media_controls(hwnd); handle_launch_files / handle_forwarded_args
│       ├── platform.rs         # Windows AUMID + Start-menu shortcut (SMTC name); call before any window
│       ├── open_files.rs       # "Open with Halo": argv → playable paths, 300ms-coalesced enqueue
│       ├── watcher.rs          # notify-based folder watcher, 2s debounce → rescan
│       ├── audio/              # rodio engine (mod.rs), EQ + stereo expander, resampler, local input,
│       │                       # media_controls.rs (souvlaki: SMTC / MPRIS / macOS)
│       ├── db/                 # connection (WAL, FKs) + versioned migrations
│       ├── scanner/            # walker, lofty metadata, art cache, waveform peaks, import_paths
│       └── commands/           # Plain functions the UI calls (formerly Tauri commands):
│                               # library, player, playlists, smart_playlists, search, eq, lyrics,
│                               # metadata_editor, lastfm, scan, folders, stats, sleep_timer, ui (prefs),
│                               # mini (mini-player prefs)
└── halo-app/                   # The iced application, package/binary `halo`
    ├── Cargo.toml              # deps + [package.metadata.packager] (installers, file associations)
    ├── build.rs                # embeds icon.ico on Windows
    ├── assets/                 # fonts (Geist, Geist Mono, lucide.ttf), lucide SVGs (filled variants,
    │                           # spinners), app icons
    ├── linux/halo.desktop.hbs  # deb desktop template (MimeType, StartupWMClass)
    └── src/
        ├── main.rs             # AUMID → single-instance forward → Backend::start → iced::daemon
        ├── app.rs              # Halo state, Message, update, view per window, subscriptions, window
        │                       # management (hide/show main, open mini, close-to-tray), bg()/fire() helpers
        ├── route.rs            # Route enum (old URL paths) + History (back/forward)
        ├── theme.rs            # Tokens (light/dark from the old index.css) + every widget style fn
        ├── icons.rs            # Lucide icon font glyphs (Icon enum → codepoint)
        ├── ui.rs               # small builders: tooltip, art, stars, toggle, modal, progress, spinners…
        ├── format.rs           # duration / relative time / plural helpers
        ├── tray.rs             # tray-icon + menu (GTK thread on Linux), events subscription
        ├── single_instance.rs  # local socket: forward argv/cwd to the running instance
        ├── widgets/            # custom widgets: textline (ellipsis/marquee), surface (hover/click
        │                       # container), popover (anchored overlay), waveform (seek bar)
        ├── components/         # shell pieces: title_bar, sidebar, now_playing (+ sleep timer),
        │                       # palette (Ctrl+K), lyrics (panel), mini (mini-player window)
        └── pages/              # ONE FILE PER PAGE / TAB
            ├── mod.rs          # Pages state, page Msg, route dispatch, load-on-enter, refresh hooks
            ├── common.rs       # virtual list/grid, scroll memory, page header helpers
            ├── tracks.rs       # shared track table (Songs / Recently Played / Most Played)
            ├── songs.rs  recently_played.rs  most_played.rs  albums.rs  folders.rs  queue.rs
            ├── library/        # mod.rs (tabs page) + cards.rs + artists, album_artists, composers,
            │                   # genres, years (each also backs its standalone route)
            ├── playlists.rs  playlist_detail.rs  smart_playlist.rs
            ├── settings/       # mod.rs (nav + row helpers) + general, appearance, playback,
            │                   # equalizer, library, advanced, lastfm
            ├── dialogs.rs      # add-to-playlist + confirm dialogs
            └── metadata/       # metadata editor: mod.rs, fields.rs, art.rs, crop.rs
```

### UI ↔ backend

The UI owns a `Backend` (cheap `Clone`). Command functions in `halo_backend::commands` are called directly, off the UI thread, through `app::bg(&backend, |b| …, |result| Message::…)` (tokio `spawn_blocking`) or `app::fire` for fire-and-forget calls. Command bodies still take `&Mutex<Connection>` / `Backend` exactly as before; `b.state::<T>()` / `b.db()` / `b.player()` replace Tauri's managed state.

Backend threads talk back through `BackendEvent`s (`PlayerState`, `LibraryChanged`, `ScanProgress`, `EqStateChanged`, `ThemeChanged`, `SleepTimer`, `RatingChanged`, `RaiseMain`, `RevealMain`) on an unbounded channel, consumed by the `backend_events` subscription in `app.rs`. Don't emit while holding the DB lock (HAZARDS).

### Paths & data

`AppPaths` resolves `dirs::data_dir()/com.saravix.halo` (DB) and `dirs::cache_dir()/com.saravix.halo` (album art, waveforms) — the same directories Tauri used, so existing libraries load unchanged (the DB stores absolute art paths).

### Database

`rusqlite` with `bundled`. One `Mutex<Connection>` inside the `Backend`. Migrations run at startup via `PRAGMA user_version`; every new `.sql` file needs a unique number **and** an `if version < N` block in `db/migrations.rs`. Normalized schema with junction tables for multi-value fields; values are split by configurable delimiters at scan time.

### State ticker (backend.rs)

A 250 ms thread drives: sleep-timer tick, `player::handle_audio_events` (track end → advance + stats; near end → crossfade), output-device follow (~1 s, `PlayerHandle::rebuild_output`), `persist_playback_progress` (~1 s), Last.fm now-playing on track change, OS media-control sync, then sends `BackendEvent::PlayerState`. Resume-on-launch and folder watching are opt-in settings (see HAZARDS for their semantics).

### Windows (iced daemon)

| Window | Notes |
|---|---|
| Main | 1200×750 (min 900×600), undecorated with custom title bar + 6px edge resize handles, `exit_on_close_request: false` — the close button hides to tray or quits per the "Close button" setting. Hidden (never closed) while the mini player is open; its HWND backs Windows SMTC. |
| Mini | 280×30, opened on demand, transparent, not resizable, always-on-top (pref, re-applied after map for Wayland), skip-taskbar on Windows, position saved (300 ms debounce) in `app_state`. Hover controls appear once the pointer moves past the 30px art. |

The tray (Play/Pause, Previous, Next, Show Halo, Mini player, Quit) and the single-instance socket both route through `Message`s.

### Look & feel

All colours come from `theme::t()` (active `Tokens`); never hard-code colours in pages — add a token or style fn to `theme.rs`. Icons are Lucide glyphs from the bundled icon font (`icons::icon(Icon::X, size)`), which inherit the button's text colour on hover; filled variants (heart, star, play) use SVGs via `ui::filled`. Text that must truncate uses `widgets::line(..)` (ellipsis) or `.marquee()` / `.looping()`.

## Testing the UI headless

See "Testing the UI headless (Xvfb)" in [docs/HAZARDS.md](docs/HAZARDS.md): `xvfb-run`/`Xvfb`, `xdotool windowfocus` before typing, a null ALSA device, and `import -window root` for screenshots.

## Development phases

| Phase | Status | Focus |
|---|---|---|
| 1–6 | ✅ Complete | Foundation, scanning, core views, playback, playlists + search, polish |
| v1.1 — User expectations | 🚧 In progress | ratings, sleep timer, mini player, lyrics, metadata editor, library tabs ✅; EQ, smart playlists, Last.fm |
| Arch A–C | ✅ Complete | `halo-core` extraction, event-driven audio, platform ports |
| iced port | ✅ Complete | Tauri/React → iced; `halo-backend` + `halo-app` crates; cargo-packager installers |
| Arch D — Android / E — iOS | ⏳ Pending | (designed against the old webview shell — needs a re-plan for iced) |
