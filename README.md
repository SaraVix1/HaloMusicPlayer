# Halo Music Player

Halo is a fast, native desktop music player for your local library. Point it at your music folders and get a clean, modern player with gapless-feeling playback, a full equalizer, lyrics, smart playlists, and Last.fm scrobbling — all running offline against your own files.

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

Halo currently runs on **Windows, macOS, and Linux**. Mobile (Android/iOS) support is on the roadmap — see [docs/plan/halo-future-features.md](docs/plan/halo-future-features.md).

## Installing

Pre-built installers are not yet published. For now, build Halo from source — see [Building from source](#building-from-source) below.

## Building from source

### Prerequisites

- **Rust** (stable toolchain — on Windows, the MSVC toolchain `stable-x86_64-pc-windows-msvc`)
- On Windows: **Visual Studio 2022+** with the **"Desktop development with C++"** workload (provides `link.exe` for the Rust MSVC toolchain)
- **Node.js** ≥ 18

### Run in development mode

```bash
npm install
npm run tauri dev
```

### Build a production binary

```bash
npm run tauri build
```

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
