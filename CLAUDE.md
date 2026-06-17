# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

**Halo** is a desktop music player built with Tauri v2 (Rust backend + React/TypeScript frontend), SQLite, and audio playback via Rust crates. See [docs/plan/halo-future-features.md](docs/plan/halo-future-features.md) for the full feature roadmap and [docs/ACTIVE_WORK.md](docs/ACTIVE_WORK.md) for what's currently in flight.

## Prerequisites

- **Rust** (stable MSVC toolchain: `stable-x86_64-pc-windows-msvc`)
- **Visual Studio 2022+** with the **"Desktop development with C++"** workload — provides `link.exe` required by the Rust MSVC toolchain. Without this workload, Git's `link.exe` shadows MSVC's and all Rust builds fail.
- **Node.js** ≥ 18

## Tech stack

| Layer | Technology |
|---|---|
| Framework | Tauri v2 |
| Frontend | React + TypeScript + shadcn/ui (base-ui) + Tailwind CSS v4 |
| Database | SQLite via `rusqlite` (bundled feature) |
| Audio playback | `rodio` |
| Metadata | `lofty` crate |
| State management | `zustand` |

## Common commands

```bash
# Install frontend dependencies
npm install

# Run in development mode (starts Vite + Rust watcher)
npm run tauri dev

# Build for production
npm run tauri build

# Type-check frontend only
npx tsc --noEmit

# Check Rust (workspace — checks all crates)
cargo check

# Run all Rust tests (workspace)
cargo test

# Run halo-core unit tests only
cargo test -p halo-core

# Run a single test by name
cargo test <test_name>

# Lint Rust
cargo clippy
```

## Architecture

### Project structure

```
halo/
├── Cargo.toml                   # [workspace] members: src-tauri, crates/*
├── crates/
│   └── halo-core/               # Pure domain logic — no I/O, no Tauri, no rodio
│       └── src/
│           ├── lib.rs           # pub mod declarations
│           ├── audio_event.rs   # AudioEvent enum (TrackFinished, NearEnd)
│           ├── media_input.rs   # MediaInput trait (Read + Seek + Send + Sync)
│           ├── now_playing.rs   # NowPlayingController trait, NowPlayingMeta, PlaybackInfo, RemoteCommand
│           ├── playback.rs      # is_played (local play-count threshold: 30s-or-50%) + should_scrobble (Last.fm threshold: 50%-or-4min, min 30s length) + unit tests
│           └── queue.rs         # RepeatMode, ShuffleHistory, next_index, should_crossfade + unit tests
├── src-tauri/
│   ├── src/
│   │   ├── main.rs              # Entry point — calls halo_lib::run()
│   │   ├── lib.rs               # Tauri builder: plugins, state, command registration, 250ms ticker (sleep timer, audio events, SMTC sync, output-device follow, persist playback progress); resume_on_launch + watcher init/reconfigure at setup; sync_media_controls; handle_remote_command; close-behavior (minimize-to-tray vs quit)
│   │   ├── watcher.rs           # FolderWatcher (notify crate): recursively watches managed folders, 2s-debounced full rescan on Create/Remove/Modify; reconfigure() on startup, toggle, and folder add/remove; set_watch_folders command
│   │   ├── tray.rs              # System tray icon, menu, and show/hide logic
│   │   ├── audio/
│   │   │   ├── mod.rs           # AudioBackend trait, PlayerHandle, Worker, rodio sink management, AudioEvent drain; rebuild_output (reopen on new OS default device, resume in place)
│   │   │   ├── eq.rs            # EqState (atomic gains + stereo/dynamic flags), 10-band biquad EqSource w/ dynamic-EQ level blend, StereoExpander (mid/side widening)
│   │   │   ├── local_input.rs   # LocalFsInput — desktop MediaInput impl (BufReader<File>)
│   │   │   ├── resampler.rs     # ResampledSource (rubato), PositionSource (sample-clock counter), device_sample_rate
│   │   │   └── media_controls.rs # souvlaki wrapper — implements NowPlayingController; spawns SMTC/MPRIS thread
│   │   ├── db/
│   │   │   ├── mod.rs           # DB connection (WAL mode, FK enforcement, migrations)
│   │   │   └── migrations.rs    # Versioned runner: 001_initial.sql (v1), 002_stats.sql (v2)
│   │   ├── scanner/
│   │   │   ├── mod.rs           # Scan orchestration + progress events
│   │   │   ├── walker.rs        # Recursive folder walk, extension filter
│   │   │   ├── metadata.rs      # lofty tag extraction, multi-value delimiter splitting
│   │   │   └── art.rs           # Album art extraction and cache
│   │   └── commands/
│   │       ├── mod.rs
│   │       ├── folders.rs       # get_folders / add_folder / remove_folder (calls watcher::reconfigure on change)
│   │       ├── library.rs       # get_tracks / get_albums / get_artists / get_album_artists / get_composers / get_genres / get_years / get_folder_tracks
│   │       ├── lyrics.rs        # get_lyrics / save_lyrics IPC commands
│   │       ├── metadata_editor.rs # get_track_full_metadata / save_track_metadata / extract_track_art / fetch_art_from_url / search_cover_art / process_art; emits library-changed after save
│   │       ├── player.rs        # Playback IPC commands, handle_audio_events, FullPlayerState; queue/shuffle/repeat via halo-core; persist_playback_progress + resume_on_launch (restores prior play/pause state, paused if unknown)
│   │       ├── playlists.rs     # Playlist CRUD + track management
│   │       ├── scan.rs          # scan_library / clear_cache / clear_database / scan settings
│   │       ├── search.rs        # Full-text search across tracks, albums, artists, playlists
│   │       ├── sleep_timer.rs   # Sleep timer state, tick(), IPC commands
│   │       ├── stats.rs         # play_count / skip_count / rating / last_played_at
│   │       ├── eq.rs            # EQ IPC: bands/bypass/presets, user presets, per-device profiles, set_eq_stereo / set_eq_dynamic; restore_eq_state + load_device_eq_if_exists
│   │       ├── lastfm.rs        # Last.fm: app API key/secret are compiled-in constants (API_KEY/API_SECRET — developer fills once; end users never enter them). Single browser token sign-in (start_auth → user authorises → complete_auth, auto-polled by Settings). Now-playing, scrobble (gated by should_scrobble), offline queue w/ retry, love/unlove (lastfm_love / lastfm_is_loved). Per-user session is memory-only (never persisted, cleared on exit) so each person on a shared machine signs into their own account
│   │       ├── smart_playlists.rs # Rule-based smart playlist CRUD + track evaluation
│   │       ├── ui.rs            # get_theme / set_theme (emits "theme-changed"); get_pref / set_pref (generic app_state key/value store) + read_close_behavior
│   │       └── window.rs        # Mini player window build, open/restore, position persistence; Linux-specific: transparent(true) + set_size after build + bottom-right default position; set_always_on_top/set_focus use .ok() (Wayland blocks both — a propagated error would skip main.hide())
│   ├── migrations/
│   │   ├── 001_initial.sql      # Full schema (idempotent CREATE TABLE IF NOT EXISTS)
│   │   └── 002_stats.sql        # rating, play_count, skip_count, last_played_at columns
│   ├── capabilities/
│   │   └── default.json         # Tauri v2 permissions: core, opener, dialog
│   └── Cargo.toml
├── src/
│   ├── components/
│   │   ├── ui/                  # shadcn/ui components (base-ui based): button, dialog, slider,
│   │   │                        # tooltip, separator, scroll-area, star-rating
│   │   ├── title-bar.tsx        # Custom drag region + window controls (decorations: false)
│   │   ├── sidebar.tsx          # Icon-only nav rail; Library icon opens /library (tabbed sub-nav inside); single Playlists entry covers manual + smart (no separate Smart Playlists icon)
│   │   ├── now-playing.tsx      # Playback controls, seek bar, volume, star rating; 2-line track info (Title·Album / Composer·Artist) with ResizeObserver marquee; time labels above seek bar; EQ shortcut icon → /settings?tab=equalizer; Last.fm love (heart) toggle shown only when connected, loved-state fetched per track
│   │   ├── mini-player.tsx      # Compact overlay window: art + 2-line marquee info / controls on hover; root div uses h-[30px] (not h-full) so WebKitGTK reports the correct 30 px preferred height to GTK on Linux
│   │   ├── sleep-timer-button.tsx # Sleep timer popover (presets, EOS mode, fade toggle, countdown)
│   │   ├── album-art.tsx        # Album art image with fallback
│   │   ├── command-palette.tsx  # Ctrl+K global search overlay
│   │   ├── virtual-grid.tsx     # Reusable virtualised grid (@tanstack/react-virtual); parentRef always mounted so ResizeObserver attaches correctly
│   │   ├── metadata-editor-dialog.tsx # Full tag editor: all standard fields + track/disc totals, comment, publisher, copyright, language; album art extract/URL/crop/resize/compress/MusicBrainz online search
│   │   ├── lyrics-panel.tsx     # Synced / plain lyrics panel; LRC file + embedded tag; auto-scroll on playback
│   │   └── add-to-playlist-dialog.tsx
│   ├── pages/
│   │   ├── all-songs.tsx        # Virtualized track table with sort, rating column, Play All / Shuffle Play
│   │   ├── albums.tsx           # Album grid (VirtualGrid)
│   │   ├── library.tsx          # Tabbed Library page: Artists / Album Artists / Composers / Genres / Years; tabs lazy-mount on first visit, stay mounted (CSS hide/show)
│   │   ├── artists.tsx          # Artist list (VirtualGrid, also reachable via /artists direct route)
│   │   ├── album-artists.tsx    # Album artist list (VirtualGrid, also reachable via /album-artists)
│   │   ├── composers.tsx        # Composer list (VirtualGrid, also reachable via /composers)
│   │   ├── genres.tsx           # Genre list (VirtualGrid, also reachable via /genres)
│   │   ├── years.tsx            # Year list (VirtualGrid, also reachable via /years)
│   │   ├── folders-view.tsx     # Folder tree
│   │   ├── playlists.tsx        # Unified playlist list: manual + smart playlists in one grid (smart badged ✨, built-ins locked); All/Manual/Smart filter; "New ▾" menu (manual vs smart). Routes each card to /playlists/:id or /smart-playlists/:id
│   │   ├── playlist-detail.tsx  # Manual playlist track list; Play / Shuffle; drag-to-reorder (native HTML5 DnD → reorder_playlist_track); rename, delete, remove track
│   │   ├── smart-playlist-detail.tsx # Smart playlist rule editor (reached from unified list); rules, match all/any, sort/limit, play/shuffle. (No standalone smart-playlists list page — merged into playlists.tsx)
│   │   ├── queue.tsx            # Current playback queue (virtualized)
│   │   └── settings.tsx         # Categorized sidebar layout (General / Appearance / Playback / Equalizer / Library / Advanced); reads ?tab= to deep-link a category (old ?tab=services maps to advanced). EQ tab: preset chips w/ active highlight, dB scale, user/device presets, stereo expander, dynamic EQ. Advanced tab: Last.fm panel (optional integration) + destructive actions
│   ├── lib/
│   │   ├── ipc.ts               # Typed invoke() wrappers for all Tauri commands
│   │   ├── player-store.ts      # Zustand store — synced from "player-state" events (250ms)
│   │   ├── theme-store.ts       # Zustand store — theme init + applyTheme()
│   │   ├── command-palette-store.ts # Zustand store — palette open/close
│   │   ├── lyrics-store.ts      # Zustand store — lyrics panel open/close/toggle
│   │   ├── library-events.ts    # Shared event bus for "library-changed" events (scan, metadata save)
│   │   ├── format.ts            # formatDuration() helper
│   │   └── utils.ts             # shadcn cn() helper
│   ├── App.tsx                  # HashRouter + Layout (TitleBar / Sidebar / Outlet / NowPlaying)
│   └── main.tsx                 # React root — mounts App or MiniPlayer based on window.__HALO_MINI__
├── docs/
│   ├── ACTIVE_WORK.md           # In-flight tracker and v1.1 queue
│   ├── HAZARDS.md               # Non-obvious gotchas and debugging notes
│   └── plan/
│       ├── halo-future-features.md        # Full feature roadmap (Tier 1–4)
│       └── halo-cross-platform-architecture.md  # Architecture design + incremental migration plan (Phases A–E)
├── package.json
├── vite.config.ts               # @tailwindcss/vite plugin + @/ alias → ./src
└── tsconfig.json                # paths: { "@/*": ["./src/*"] }
```

### Frontend ↔ backend communication

All cross-boundary calls go through Tauri IPC commands defined in `src-tauri/src/commands/`. The frontend invokes them via typed wrappers in `src/lib/ipc.ts`. Add new commands there and register them in `lib.rs`'s `invoke_handler!` macro.

### Database

`rusqlite` with `features = ["bundled"]` (no system SQLite required). The connection is opened once in `setup()`, wrapped in `Mutex<Connection>`, and shared via Tauri's `State`. Migrations run at startup via a versioned runner in `migrations.rs` using `PRAGMA user_version` — safe to add new migration files without touching old ones.

Normalized schema with junction tables for multi-value fields (artist, genre, composer). Multi-value metadata is split by configurable delimiters at scan time.

### State ticker

A 250ms background thread in `lib.rs` drives:
1. `sleep_timer::tick()` — deadline countdown, fade-out volume ramp
2. `player::handle_audio_events()` — drains `AudioEvent`s from `PlayerHandle`: `TrackFinished` advances the queue and records play stats; `NearEnd` starts crossfade if configured
3. `sync_media_controls()` — pushes updated metadata/playback state to the OS media panel (SMTC/MPRIS) when track or status changes
4. Output-device follow — polls the OS default device (~1s cadence) and calls `PlayerHandle::rebuild_output()` on a real change
5. `player::persist_playback_progress()` — writes current index / position / status to the `app_state` key/value store so playback can be resumed next launch
6. Emits `"player-state"` (carrying `FullPlayerState` including sleep timer info) — all windows receive it

Crossfade and track-advance are **event-driven** (Phase B): the audio worker fires `AudioEvent::NearEnd` / `AudioEvent::TrackFinished` via a channel; the ticker drains them — no polling of position or flags.

**Resume on launch** (opt-in via the Playback setting): at startup `player::resume_on_launch` reads the persisted index/position/status and, if enabled, reloads the saved track and seeks to position. It restores the prior play/pause state, defaulting to **paused** when the status is unknown to avoid surprise audio on launch.

**Folder watching** (opt-in via the Library setting): `watcher.rs` watches all managed folders recursively (`notify` crate). Create/Remove/Modify events are coalesced through a 2s debounce, then trigger a full `run_scan`. `watcher::reconfigure` re-applies the live watch set on startup, on toggle, and whenever folders are added/removed.

### shadcn/ui version note

This project uses the newer shadcn/ui that targets `@base-ui/react` instead of `@radix-ui/react`. Key differences:
- `TooltipProvider` takes `delay` (not `delayDuration`)
- `TooltipTrigger` renders a native `<button>` directly — no `asChild` prop
- Components are in `src/components/ui/`

### Windows

Two windows — both `decorations: false`:

| Window | Label | Notes |
|---|---|---|
| Main | `main` | Full player UI. Hidden (not closed) when X is clicked or mini player opens. `skip_taskbar` toggled dynamically — false when visible, true when hidden. |
| Mini player | `mini` | 280×30 floating overlay, `always_on_top`, `skip_taskbar: true` always. Controls appear on hover past the album art area (30px threshold). Position persisted in DB. Built with `transparent(true)` on all platforms (on Linux this is required to bypass the Wayland compositor minimum height; on Windows/macOS it is a no-op since the content covers the window fully). |

The tray handles show/hide for both windows. `tray::show_main_window` hides the mini player and restores the main window's taskbar entry.

## Development phases

| Phase | Status | Focus |
|---|---|---|
| 1 — Foundation | ✅ Complete | Tauri setup, SQLite, app shell, settings/folder management |
| 2 — Scanning | ✅ Complete | lofty metadata, folder walker, album art, progress events |
| 3 — Core views | ✅ Complete | Songs table, albums grid, artists, genres, folders tree |
| 4 — Playback | ✅ Complete | Audio engine, queue, now-playing wired up |
| 5 — Playlists + search | ✅ Complete | Playlist CRUD, Ctrl+K global search |
| 6 — Polish | ✅ Complete | Media keys, tray, themes, virtualization, crossfade |
| v1.1 — User expectations | 🚧 In progress | Play counts & ratings ✅, Sleep timer ✅, Mini player polish ✅, Lyrics ✅, Metadata editor ✅, Library tabbed view + VirtualGrid ✅, Now-playing UI polish ✅; EQ, smart playlists, Last.fm remaining |
| Arch A — Extract core | ✅ Complete | Cargo workspace; `halo-core` crate with `RepeatMode`, `ShuffleHistory`, `next_index`, `should_crossfade`, `is_played`; 23 unit tests |
| Arch B — Event-driven audio | ✅ Complete | `PositionSource` (sample-clock position), `AudioEvent` channel replaces `finished_flag`/`crossfading` atomics, `MediaInput` trait + `LocalFsInput`, natural-end play-stat bug fixed |
| Arch C — Platform ports | ✅ Complete | `NowPlayingController` trait in `halo-core`, `RemoteCommand` unifies hotkey vocab, `MediaControlsHandle` implements the trait, `handle_remote_command` replaces `handle_hotkey` |
| Arch D — Android | ⏳ Pending | `MediaStoreSource`, Android `MediaSession` plugin, audio-focus handling, responsive frontend shell, `sources` schema migration |
| Arch E — iOS | ⏳ Pending | Document-picker/MediaLibrary source, `MPNowPlayingInfoCenter`, background-audio mode |
