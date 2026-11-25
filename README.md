# Halo Music Player

> **Beta:** Halo is currently in beta. Releases are labelled "(Beta)" — expect rough edges, and please report any bugs you hit.

Halo is a fast, native desktop music player for your local library, written entirely in Rust with a native [iced](https://iced.rs) UI — no webview, no Node.js. Point it at your music folders and get a clean, modern player with gapless-feeling playback, a full equalizer, lyrics, smart playlists, and Last.fm scrobbling — all running offline against your own files.

## Features

### Library
- Scan one or more folders recursively; auto-detects new/changed/removed files via an optional folder watcher
- Browse by Songs, Albums, Artists, Album Artists, Composers, Genres, Years, or raw Folders
- Virtualized lists/grids — smooth scrolling even on large libraries
- Full-text search across tracks, albums, artists, and playlists (Ctrl+K command palette)
- Multi-value tags (e.g. multiple artists/genres) split automatically at scan time

### Playback
- Gapless queue with shuffle (with proper "previous" history) and repeat (off/all/one)
- Crossfade between tracks
- Resume playback on launch (restores track, position, and play/pause state)
- Follows your OS default output device automatically if it changes (e.g. switching headphones)
- Sleep timer — stop after N minutes or at the end of the current song, with optional fade-out
- Media key support and OS media panel integration (Windows SMTC, macOS, Linux MPRIS) including lock-screen artwork

### Sound
- 10-band graphic equalizer with built-in presets (Rock, Pop, Jazz, Classical, Electronic, Bass Boost, Treble Boost, Vocal, Acoustic) plus your own custom presets
- Per-output-device EQ profiles, auto-loaded when you switch devices
- Stereo expander (mid/side widening) and dynamic, level-adaptive EQ

### Lyrics
- Automatic lyrics from `.lrc` files or embedded tags
- Synced view that highlights and auto-scrolls the current line
- Plain-text fallback and manual lyrics editing

### Organizing your music
- Star ratings, play counts, skip counts, and last-played tracking
- Manual playlists with drag-to-reorder
- Smart playlists — rule-based, auto-updating playlists (by artist, genre, year, rating, play count, date added, duration, etc.), with built-ins like Recently Added, Most Played, Top Rated, Never Played, and Recently Played
- Full metadata editor: edit all standard tags, plus album art (extract from file, paste a URL, search MusicBrainz, crop/resize/compress) — writes changes back to the actual file

### Last.fm
- Connect your account via browser sign-in (no app credentials required from you beyond a key/secret you can supply in Settings)
- Now-playing updates and scrobbling (using Last.fm's standard threshold)
- Love/unlove tracks, synced with your Last.fm account
- Offline scrobble queue with automatic retry when you're back online

### Windows & UI
- Customizable light/dark themes
- Compact, always-on-top mini player (280×30) with hover-to-reveal controls — never clutters your taskbar
- System tray integration with minimize-to-tray or quit-on-close behavior

## Platform support

Halo currently runs on **Windows, macOS, and Linux**. On Linux the target is **Debian 12 (bookworm) and Debian 13 (trixie)**: packages are built on Debian 12, and every release's `.deb` is install-tested on both. The tray icon works on KDE Plasma, COSMIC, Deepin, Cinnamon, XFCE, Budgie and MATE. GNOME needs the AppIndicator extension (`sudo apt install gnome-shell-extension-appindicator`, then log out and in); without a tray, the close button minimizes instead of hiding to the tray. Mobile (Android/iOS) support is on the roadmap — see [docs/plan/halo-future-features.md](docs/plan/halo-future-features.md).

## Installing

Beta installers for Windows (setup `.exe` and `.msi`), macOS (universal `.dmg`), and Linux (`.AppImage`, `.deb`, `.rpm`) are published on the [Releases](https://github.com/SaraVix1/HaloMusicPlayer/releases) page; the newest regular release is marked "Latest". You can also build Halo from source — see [Building from source](#building-from-source) below.

### Versioning

Halo uses `major.minor.patch` versions starting at `0.1.0`. Every push to `main` bumps the patch number automatically (`0.1.0` → `0.1.1` → `0.1.2` …). To set a version by hand, run `python3 scripts/bump-version.py <version>` and include `[skip bump]` in the commit title.

To publish a release, open **Actions → Release → Run workflow** on `main`. It builds the current version and publishes it as `v<version> (Beta)`. Pushing a `v*` tag works too.

To publish a test build from any other branch, pick that branch in **Run workflow**. It is released as `v<version>-<branch>.<run number>` (for example `v0.1.3-iced-port.12`), a pre-release that never becomes "Latest".

**Run workflow** also shows a checkbox per package — Windows setup `.exe` / `.msi`, Linux `.deb` / AppImage / `.rpm` / Arch package, macOS `.dmg` — so you can build only what you need. Only the operating systems with a ticked package are built. Pushing a `v*` tag builds everything except the Arch package.

## Building from source

### Prerequisites

- **Rust** (stable toolchain — on Windows, the MSVC toolchain `stable-x86_64-pc-windows-msvc`)
- On Windows: **Visual Studio 2022+** with the **"Desktop development with C++"** workload
- On Linux: `pkg-config libasound2-dev libdbus-1-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libgtk-3-dev libayatana-appindicator3-dev libxdo-dev`

### Run in development mode

```bash
cargo run -p halo
```

### Build a production binary and installers

```bash
cargo build --release -p halo
cargo install cargo-packager --locked
cargo packager --release -p halo
```

Installers are written to `target/release/`. For an `.rpm` (Fedora, openSUSE, …), after the release build:

```bash
cargo install cargo-generate-rpm --locked
cargo generate-rpm -p crates/halo-app
```

It is written to `target/generate-rpm/`.

## Getting started

1. Launch Halo and open **Settings → Library** to add the folder(s) containing your music.
2. Halo scans your files and builds your library (Songs, Albums, Artists, etc.).
3. Optionally enable the folder watcher in Settings so new/changed files are picked up automatically.
4. Optionally connect Last.fm in **Settings → Advanced** to scrobble what you play.
5. Use the EQ icon in the now-playing bar, or **Settings → Equalizer**, to tune your sound.

## Project status

Halo is under active development. See [docs/ACTIVE_WORK.md](docs/ACTIVE_WORK.md) for what's currently being worked on and [docs/plan/halo-future-features.md](docs/plan/halo-future-features.md) for the full feature roadmap.

## Contributing / development docs

For architecture details, codebase layout, and contributor guidance, see [CLAUDE.md](CLAUDE.md).

## License

Halo is free software: you can redistribute it and/or modify it under the terms of the **GNU General Public License, version 3 or (at your option) any later version** (`GPL-3.0-or-later`). It is distributed in the hope that it will be useful, but **without any warranty**; see [LICENSE](LICENSE) for the full text.

Bundled third-party code and assets keep their own licenses:

- [souvlaki](vendor/souvlaki) (vendored, patched): MIT — [vendor/souvlaki/LICENSE](vendor/souvlaki/LICENSE)
- Geist and Geist Mono fonts: SIL Open Font License 1.1 — [crates/halo-app/assets/fonts/LICENSE.txt](crates/halo-app/assets/fonts/LICENSE.txt)
- Lucide icons: ISC — [crates/halo-app/assets/lucide/LICENSE](crates/halo-app/assets/lucide/LICENSE)
