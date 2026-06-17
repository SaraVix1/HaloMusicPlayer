# Active Work

What's in flight right now, what's queued next, what's paused. The roadmap with full feature specs lives in [plan/halo-future-features.md](plan/halo-future-features.md) — this file is the lighter, faster-moving tracker.

Keep entries short. Link to the full spec rather than duplicating it.

---

## In Progress

_Nothing currently in flight. Move items here from **Next Up** when you start them._

<!--
Template:
- **<feature name>** — short status note. Links: [spec](plan/halo-future-features.md#section), [PR or branch].
-->

---

## Architecture — Cross-Platform Refactor

Tracked separately from v1.1 features. Spec: [plan/halo-cross-platform-architecture.md](plan/halo-cross-platform-architecture.md).

- ~~**Phase A — Extract core**~~ — ✅ Complete. Cargo workspace; `halo-core` crate with pure queue/playback logic; 23 unit tests. No behavior change.
- ~~**Phase B — Event-driven audio**~~ — ✅ Complete. `PositionSource` sample-clock replaces `Instant` drift. `AudioEvent` channel (`TrackFinished`, `NearEnd`) replaces `finished_flag`/`crossfading` atomics. `MediaInput` trait + `LocalFsInput`. Pre-existing bug fixed: play-stats now recorded on natural track end.
- ~~**Phase C — Platform ports (desktop)**~~ — ✅ Complete. `NowPlayingController` trait + `RemoteCommand` in `halo-core`. `MediaControlsHandle` implements the trait. `handle_remote_command` replaces `handle_hotkey`. Desktop (Windows SMTC / macOS / Linux MPRIS) fully unified.
- **Phase D — Android** — ⏳ Not started. `MediaStoreSource`, Android `MediaSession` Tauri plugin, audio-focus/interruption handling, responsive frontend shell + capability gating, `sources` schema migration.
- **Phase E — iOS** — ⏳ Not started. Document-picker source, `MPNowPlayingInfoCenter` + remote command center, background-audio mode.

---

## Next Up (v1.1 — Tier 1)

Pulled from [plan/halo-future-features.md](plan/halo-future-features.md). Mini player (§1.7) and command palette (§3.2) are already shipped and excluded here.

**Recent polish (not in original spec):**
- ~~Settings reorganization~~ — ✅ Categorized sidebar (General / Appearance / Playback / Equalizer / Library / Advanced). New: close-behavior (minimize-to-tray vs quit), resume-on-launch + watch-folders toggles, auto-scan-on-add. Generic `get_pref`/`set_pref` key/value store added. (Last.fm now lives under Advanced as an optional integration — the standalone Services tab was removed since it held nothing else.)
- ~~Resume on launch~~ — ✅ Backend wired. The 250ms ticker persists current index / position / status to `app_state` (`player::persist_playback_progress`); at startup `player::resume_on_launch` reloads the saved track and seeks to position. Restores the prior play/pause state, defaulting to **paused** when the status is unknown (avoids surprise audio). Spec [§Tier 3 resume].
- ~~Watch folders~~ — ✅ Backend wired. [src-tauri/src/watcher.rs](../src-tauri/src/watcher.rs): `notify` crate watches all managed folders recursively; Create/Remove/Modify events are coalesced through a 2s debounce, then trigger a full `run_scan` + `library-changed`. `watcher::reconfigure` is called on startup, on the `set_watch_folders` toggle, and on folder add/remove. Full rescan per change (debounced) — fine for small libraries; incremental scan is a future 50k+ improvement.
- ~~Unified Playlists~~ — ✅ Manual + smart playlists now share one `/playlists` list page and one sidebar entry (smart badged ✨, built-ins locked; All/Manual/Smart filter; "New ▾" picks manual vs smart). Detail editors and DB tables stay separate; `/smart-playlists` list redirects to `/playlists`. Manual playlist detail gained a Shuffle button and native drag-to-reorder (replacing ↑↓ arrows).
- ~~Follow system output device~~ — ✅ When the OS default playback device changes, the audio stream reopens on the new device and resumes the current track in place (`PlayerHandle::rebuild_output`).
- ~~Mini player UX~~ — ✅ Controls visible only past album art on hover; timer labels draggable; expand button distinct background; padding removed.
- ~~Mini player taskbar~~ — ✅ Mini window never appears in taskbar; main window taskbar entry follows visibility (hidden when X-closed or mini active, restored when main shown). Tray "Show Halo" hides mini player.
- ~~Shuffle Previous~~ — ✅ In-memory history stack; Previous in shuffle returns actual last-played track.
- ~~Library tabbed view~~ — ✅ Single `/library` route with Artists / Album Artists / Composers / Genres / Years tabs. Tabs lazy-mount on first visit and stay mounted (CSS hide/show, not unmount). VirtualGrid virtualization across all list pages.
- ~~All Songs play buttons~~ — ✅ Play All (sequential from first) and Shuffle Play (random start, shuffle mode on) buttons in All Songs header.
- ~~Now-playing bar UI~~ — ✅ 2-line track info (Title · Album / Composer · Artist) with ResizeObserver-driven marquee; time labels moved above seek bar; wider track info section (w-80); improved mode button visibility with active indicator dot; rating moved before shuffle button.


- ~~**Equalizer**~~ — ✅ Shipped. 10-band graphic EQ, built-in + user presets (active highlighted), per-device profiles, bypass. Added stereo expander (mid/side widening) and Dynamic EQ (level-adaptive blend). UI reworked: preset chips, dB scale, now-playing EQ shortcut. [Spec §1.1](plan/halo-future-features.md#11-equalizer). Note: per-band dynamic EQ remains a future enhancement.
- ~~**Lyrics display**~~ — ✅ Shipped. Embedded tag + .lrc file detection, synced highlight with auto-scroll, plain fallback, manual edit. Toggle via Mic button in now-playing bar.
- ~~**Metadata editor**~~ — ✅ Shipped. Full tag editor: title, artist, album artist, composer, genre, year, track/disc number + totals, comment, publisher, copyright, language. Album art pipeline: extract from file, load from URL, MusicBrainz online search, crop UI, resize to 750×750 + JPEG compress to ≤350 KB. Writes tags back to file (lofty) + updates DB/junction tables. Emits `library-changed` so all views refresh. Edit button (pencil icon) on track rows in All Songs.
- ~~**Play count & ratings**~~ — ✅ Shipped. 5-star rating, play/skip counts, last played timestamp, reset stats in Settings.
- ~~**Smart playlists**~~ — ✅ Shipped. Rule-based auto-playlists: text/number/date rules across title, artist, album, genre, composer, year, rating, play/skip count, duration, date added, last played; AND/OR match modes; sort + limit; injection-safe SQL compiler. 5 built-ins seeded by migration (Recently Added, Most Played, Top Rated, Never Played, Recently Played). Detail page auto-refreshes its track list on `library-changed`. [Spec §1.5](plan/halo-future-features.md#15-smart-playlists).
- ~~**Sleep timer**~~ — ✅ Shipped. Countdown presets (5–60 min), end-of-song mode, 10s fade-out option, live countdown in now-playing bar.
- ~~**Last.fm scrobbling**~~ — ✅ Shipped + reworked. Now-playing, offline queue with retry. Rework (2026-06-02): (1) **user-entered app key** — API key + shared secret entered in Settings → Advanced and persisted in `app_state` (`lastfm_set_credentials`); not hardcoded (was briefly a compiled-in constant, reverted on user request). Until both are set the panel shows the credentials form; (2) **two-option auth** — "Connect with browser" fetches a token then shows an **Authenticate** step with **Open in browser** + **Copy link** (clipboard via `navigator.clipboard`); auto-poll still finishes hands-free; (3) **correct scrobble threshold** — `halo_core::playback::should_scrobble` (50%-or-4min, min 30s) replaces lenient `is_played` for scrobble gating; (4) **Love/unlove** — `track.love`/`track.unlove` + `track.getInfo` loved-state, heart toggle in now-playing (shown only when connected); (5) panel is a self-contained `LastFmPanel` under Settings → Advanced; per-user session memory-only. [Spec §1.8](plan/halo-future-features.md#18-lastfm-scrobbling).

---

## Backlog

Tracked at tier level — promote individual items to **Next Up** when v1.1 closes out.

- **v1.2 — Library quality.** Duplicate finder, artwork fetcher, ReplayGain, CUE, visualizer, drag-drop, multi-select, casting. See [Tier 2](plan/halo-future-features.md#tier-2--highly-useful-v12).
- **v1.3 — Power users.** Custom global hotkeys (media keys already shipped), recently added view, most played view, output format settings (WASAPI/ASIO/CoreAudio), playlist import/export. (Folder watcher ✅ shipped early — see Recent polish above.) See [Tier 3](plan/halo-future-features.md#tier-3--power-user-features-v13).
- **v2.0 — Major expansion.** Podcasts, dynamic color theming, internet radio, Discord Rich Presence, backup/restore, statistics dashboard, album/artist bios, karaoke mode. See [Tier 4](plan/halo-future-features.md#tier-4--nice-to-have-v20).

---

## Blocked / Paused

_Nothing blocked._

<!--
Template:
- **<feature name>** — blocked on <reason>. Action: <what unblocks it>.
-->

---

## Cross-cutting tracks

Not features per se, but threads that need attention as v1.1 grows. From [plan/halo-future-features.md](plan/halo-future-features.md#cross-cutting-concerns-to-address-with-these-features):

- DB migrations for ratings / play counts / smart-playlist rules / scrobble queue
- ~~Settings page reorganization (tabs/sections) as the list grows~~ — ✅ Done (categorized sidebar)
- Large-library performance (50k+ tracks)
- i18n decision point (commit early or punt to v2)
- Accessibility pass (ARIA, keyboard nav, screen readers)
