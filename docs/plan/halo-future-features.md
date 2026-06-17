# Halo — future features roadmap

Features planned beyond the core v1 release. Organized by priority tier and grouped into release phases.

---

## Tier 1 — Essential (v1.1)

These are features users commonly expect in a modern music player. Plan to ship these in the first major update after v1.

### 1.1 Equalizer ✅ Shipped
- 10-band graphic EQ (biquad peaking filters)
- Built-in presets: Flat, Rock, Pop, Jazz, Classical, Electronic, Bass Boost, Treble Boost, Vocal, Acoustic — shown as chips with the active preset highlighted; "Current: <name>/Custom" label; dB scale axis
- Custom user presets (save/load/delete), active user preset highlighted
- Per-output-device EQ profiles (auto-loaded when the OS default device changes)
- Bypass toggle
- Stereo expander (mid/side widening, width 0–3×) and Dynamic EQ (level-adaptive blend) — global, persisted
- Reachable via EQ icon in the now-playing bar (`/settings?tab=equalizer`)
- ~~Per-band dynamic EQ~~ — current Dynamic EQ eases the whole curve by signal level, not per-band

### 1.2 Lyrics display ✅ Shipped
- LRC file detection (alongside audio file or in dedicated lyrics folder)
- Embedded lyrics from ID3 tags
- Synced lyrics view: highlight current line as song plays
- Plain (unsynced) lyrics fallback
- Manual lyrics input/edit
- Toggle lyrics panel from now-playing bar

### 1.3 Metadata editor ✅ Shipped
- Single track editor with all standard tags
- Edit: title, artist, album, album artist, composer, genre, year, track number, disc number, comment, publisher, copyright, language
- Album art: extract from file, load from URL, MusicBrainz online search, crop/resize/compress
- Write changes back to file via `lofty`; updates DB + junction tables; emits `library-changed`
- ~~Batch editor~~ / ~~undo~~ — not shipped

### 1.4 Play count & ratings ✅ Shipped
- 5-star rating per track
- Auto-incrementing play count
- Last played timestamp
- Skip count tracking
- Reset stats option in Settings

### 1.5 Smart playlists
- Rule-based auto-generated playlists
- Built-in: Recently Added, Most Played, Top Rated, Never Played, Recently Played
- Custom rules: artist, genre, year, rating, play count, date added, duration
- Combine rules with AND/OR logic
- Auto-update or manual refresh

### 1.6 Sleep timer ✅ Shipped
- Stop after X minutes (5, 10, 15, 30, 45, 60, 90, custom)
- Stop after current song (end-of-song mode)
- Fade-out on stop toggle
- Live countdown in now-playing bar

### 1.7 Mini player mode ✅ Shipped
- Compact 280×30 floating overlay window (always-on-top)
- Album art, 2-line marquee (title/artist), play/pause/skip controls on hover
- Click to restore full window; tray "Show Halo" hides mini and restores main
- Never appears in taskbar

### 1.8 Last.fm scrobbling
- OAuth authentication
- Auto-scrobble after configurable threshold (50% or 4 min)
- Now playing notification
- Love/unlove tracks synced to Last.fm
- Offline scrobble queue with retry

---

## Tier 2 — Highly useful (v1.2)

### 2.1 Duplicate finder
- Detect duplicates by: file hash, title+artist+duration, fuzzy match
- Review duplicates side-by-side
- Bulk remove or merge options
- Keep highest quality (bitrate/format priority)

### 2.2 Missing artwork fetcher
- Scan library for tracks/albums without art
- Auto-fetch from MusicBrainz, Cover Art Archive, Last.fm
- Preview before apply
- Bulk operation with progress bar
- Embed in file or save as folder.jpg option

### 2.3 Audio normalization
- ReplayGain track and album mode
- Scan and write ReplayGain tags to files
- Real-time loudness normalization (no tag rewriting needed)
- Per-output-device normalization toggle

### 2.4 CUE sheet support
- Parse .cue files for single-file albums
- Treat virtual tracks as individual playable items
- Display track listings from CUE in album view

### 2.5 Visualizer
- Spectrum analyzer (bars)
- Waveform display
- Circular/radial visualizer
- Background mode (subtle behind album art)
- Color sync with album art (see 4.2)

### 2.6 Drag-and-drop
- Drop audio files onto app → play or add to queue prompt
- Drop folders → add to managed folders prompt
- Drop tracks onto playlist in playlists page
- Reorder queue items via drag

### 2.7 Multi-select operations
- Ctrl/Shift click for multi-select in song lists
- Batch actions: add to queue, add to playlist, edit metadata, delete, rate
- Select all / invert selection

### 2.8 Cast/streaming output
- Bluetooth device picker
- Chromecast support
- AirPlay support (macOS/iOS targets)
- DLNA/UPnP rendering

---

## Tier 3 — Power user features (v1.3)

### 3.1 Global hotkeys & media keys ✅ Partially shipped
- ✅ Media key support (play/pause, next, previous, seek via OS SMTC/MPRIS)
- ✅ OS media panel integration (Windows SMTC, macOS, Linux MPRIS) via `souvlaki` + `NowPlayingController` trait
- ~~Customizable global hotkeys~~ — not yet shipped
- ~~Per-action shortcut configuration~~ — not yet shipped

### 3.2 Command palette ✅ Shipped
- Ctrl+K global search overlay
- Jump to song, album, artist, playlist from anywhere

### 3.3 Recently added view
- Songs added in last 7 / 30 / 90 days
- Sort by date added descending
- Quick access from sidebar or All Songs filter

### 3.4 Most played view
- Top tracks by play count
- Time period filters: all time, this month, this year
- Top artists and top albums variations

### 3.5 Output format settings
- Bit depth selection (16, 24, 32-bit)
- Sample rate selection or follow-source
- Exclusive mode (WASAPI on Windows)
- ASIO driver support (Windows)
- CoreAudio settings (macOS)

### 3.6 Playlist import/export
- Export to M3U, M3U8, PLS, XSPF
- Import from M3U, M3U8, PLS
- Relative vs absolute path option on export
- Bulk export multiple playlists

### 3.7 Folder watcher (optional)
- Off by default (respects your manual-scan preference)
- Optional toggle per managed folder
- Auto-detect new files
- Auto-detect removed/renamed files
- Notification on changes

---

## Tier 4 — Nice to have (v2.0)

### 4.1 Podcast support
- Subscribe to RSS feeds
- Episode list per podcast
- Auto-download new episodes
- Playback speed control (0.5x to 3x)
- Position memory per episode

### 4.2 Color extraction & dynamic UI
- Extract dominant colors from current album art
- Tint now-playing bar background
- Optional full-app theming based on current track
- Smooth color transitions

### 4.3 Internet radio
- Add radio stations by URL or browse directory
- Recently played stations
- Now-playing metadata from stream

### 4.4 Discord Rich Presence
- Show currently playing track on Discord profile
- Toggle on/off in Settings
- Privacy: hide on private playlists option

### 4.5 Backup & restore
- Export full database to file
- Export Settings and preferences
- Import on new machine or after clear
- Auto-backup option (daily/weekly)

### 4.6 Statistics dashboard
- Total listening time
- Top artists, albums, genres (by play count and listening time)
- Listening trends over time (chart)
- Most active listening hours/days
- Yearly recap (Spotify Wrapped style)

### 4.7 Album/artist bios
- Fetch from MusicBrainz and Wikipedia
- Show bio in artist/album detail pages
- Cache locally

### 4.8 Now playing system integration ✅ Shipped (desktop)
- ✅ Windows: SMTC with album art thumbnail via `StorageFile`/`RandomAccessStreamReference`
- ✅ macOS / Linux: MPRIS D-Bus via `souvlaki`
- ✅ Lock screen / media panel artwork
- Android/iOS media session — covered by Arch Phase D/E

### 4.9 Karaoke mode (experimental)
- Vocal removal via center channel subtraction or AI model
- Quality varies by source
- Mark as experimental

---

## Release strategy

| Version | Focus | Features |
|---|---|---|
| **v1.0** | Core player | Original plan (Phases 1-6) |
| **v1.1** | User expectations | Tier 1 — ✅ lyrics, metadata editor, ratings, sleep timer, mini player; remaining: EQ, smart playlists, Last.fm |
| **v1.2** | Library quality | Tier 2 — duplicates, artwork fetcher, ReplayGain, CUE, visualizer, drag-drop, multi-select, casting |
| **v1.3** | Power users | Tier 3 — custom hotkeys, recently added, most played, output formats, playlist I/O, folder watcher |
| **v2.0** | Major expansion | Tier 4 — podcasts, dynamic theming, internet radio, Discord, stats |

---

## Cross-cutting concerns to address with these features

- **Database migrations**: New tables for ratings, play counts, smart playlist rules, scrobble queue
- **IPC commands**: New Tauri commands for each feature area
- **Settings page expansion**: Tabs/sections to organize growing settings list
- **Localization**: Consider i18n setup early if international users targeted
- **Accessibility**: ARIA labels, keyboard navigation, screen reader support throughout
- **Performance**: Large library handling (50k+ tracks) — virtualized lists, indexed queries
- **Error reporting**: Optional crash reporting / telemetry (opt-in)
