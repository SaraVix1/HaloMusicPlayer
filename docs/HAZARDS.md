# Hazards

Non-obvious traps in this codebase. When you hit a footgun that took non-trivial time to diagnose, add it here so the next person (or future you) doesn't pay the same cost.

Each entry: **symptom → cause → fix**.

---

## MSVC link.exe shadowed by Git's link.exe

**Symptom.** `cargo build` / `cargo check` fails with cryptic linker errors, or links the wrong binary on Windows.

**Cause.** Git for Windows ships its own `link.exe` (a symlink utility). If Git's `usr\bin` is on `PATH` before Visual Studio's MSVC `link.exe`, Cargo invokes the wrong one.

**Fix.** We sidestep this entirely by configuring `linker = "rust-lld"` in [.cargo/config.toml](../.cargo/config.toml) — rust-lld is bundled with rustup and resolved via the toolchain, not PATH, so Git's `link.exe` can't interfere. Don't remove that config without a reason. If you do need MSVC `link.exe`, install Visual Studio's **"Desktop development with C++"** workload and use the **Developer PowerShell for VS** (not a plain shell).

---

## Cargo won't re-embed a new `icon.ico`

**Symptom.** You replaced `crates/halo-app/assets/icons/icon.ico`, rebuilt, and the old icon is still shown in the taskbar / Explorer.

**Cause.** [crates/halo-app/build.rs](../crates/halo-app/build.rs) embeds `icon.ico` as a Windows resource (`winresource`). It declares `rerun-if-changed`, but Windows also caches shell icons aggressively.

**Fix.** `cargo clean -p halo` to force a full rebuild. The helper script [docs/process/clean-icon-cache.bat](process/clean-icon-cache.bat) does this plus flushes the Windows shell icon cache. See [docs/process/updating-app-icon.md](process/updating-app-icon.md).

---

## SMTC shows "Unknown app" without an AUMID shortcut

**Symptom.** Windows Media Controls / SMTC panel shows "Unknown app" instead of "Halo" when something is playing.

**Cause.** Windows SMTC looks up the display name by matching the process's AppUserModel.ID against a Start Menu shortcut whose `System.AppUserModel.ID` property matches the same string. No shortcut → no name.

**Fix.** `set_app_user_model_id()` in [crates/halo-backend/src/platform.rs](../crates/halo-backend/src/platform.rs) (called first thing in `main`) installs the shortcut itself (the same approach Spotify / Discord / VS Code use). Don't remove it.

---

## SQLite GROUP_CONCAT: outer query cannot reference an inner alias by table prefix

**Symptom.** A query like `SELECT GROUP_CONCAT(a.name, '||') FROM (SELECT a.name FROM … JOIN artists a …)` fails with `no such column: a.name`.

**Cause.** The outer `SELECT` sees only the column *name* produced by the subquery, not the original table alias `a`. The alias `a` is scoped to the inner query and invisible outside it.

**Fix.** Use the bare column name in the outer GROUP_CONCAT: `SELECT GROUP_CONCAT(name, '||') FROM (SELECT a.name FROM …)`. The derived column is named `name`, not `a.name`. See [crates/halo-backend/src/commands/library.rs](../crates/halo-backend/src/commands/library.rs).

---

## Cargo ignores `[profile.*]` in workspace member packages

**Symptom.** A build prints `warning: profiles for the non root package will be ignored, specify profiles at the workspace root`, and dev-build tuning you put in a member `Cargo.toml` (faster incremental settings, debug level, etc.) silently has no effect.

**Cause.** In a Cargo workspace, profile settings are only honored in the **root** manifest. A `[profile.dev]` (or any `[profile.*]`) block inside a member package is parsed but ignored — so the tuning appears configured while doing nothing.

**Fix.** Put all `[profile.*]` blocks in the root [Cargo.toml](../Cargo.toml), not in [crates/halo-backend/Cargo.toml](../crates/halo-backend/Cargo.toml) or any `crates/*` member. There's one workspace `target/` dir, so one set of profiles governs everything.

---

## rodio output stays on the startup device; it does not follow the OS default

**Symptom.** You switch the Windows default playback device (headphones → speakers, BT, etc.) while a track is playing. The OS default changes, but Halo keeps sending audio to the *original* device.

**Cause.** `OutputStream::try_default()` opens whatever device is default **at that moment** and binds the stream to it for life. rodio/cpal never migrate an open stream when the system default changes — there's no automatic follow.

**Fix.** Detect the change and rebuild the stream. The 250 ms ticker in [crates/halo-backend/src/backend.rs](../crates/halo-backend/src/backend.rs) polls `get_current_device_name()` (~1 s cadence) and, on a real change, calls `PlayerHandle::rebuild_output()`. That sends `Command::RebuildOutput` to the audio worker, which reopens `OutputStream::try_default()` on the new device, **recomputes the sample rate** (the new device may differ), re-decodes the current track from `current_path`, and seeks back to the saved position — preserving paused state and volume. Skip the very first detection at startup (`last_device` empty) since the stream already opened on the correct default. See `rebuild_output` in [crates/halo-backend/src/audio/mod.rs](../crates/halo-backend/src/audio/mod.rs).

---

## `notify` watcher stops silently if its handle is dropped

**Symptom.** Folder watching "works" right after toggling it on, but stops firing rescans later — or never fires at all if the watcher was created inside a function and not stored anywhere. No error, no panic; filesystem changes are simply ignored.

**Cause.** `notify`'s `RecommendedWatcher` watches only as long as the handle is alive. The OS subscription is torn down in its `Drop` impl. A `let watcher = notify::recommended_watcher(...)` that goes out of scope unsubscribes immediately.

**Fix.** Keep the watcher handle in long-lived state. We store it in `FolderWatcher.watcher: Mutex<Option<RecommendedWatcher>>` (held by the `Backend`). Setting that `Option` to `None` is also how we **stop** watching when the toggle is turned off — dropping the handle is the intended off-switch. `reconfigure()` rebuilds the handle from scratch on every change (startup, toggle, folder add/remove) rather than mutating an existing one. See [crates/halo-backend/src/watcher.rs](../crates/halo-backend/src/watcher.rs).

---

## Resume-on-launch defaults to paused only when prior status is unknown

**Symptom.** You expect resume-on-launch to always come up paused (a safe default), but Halo sometimes opens already **playing** audio on launch.

**Cause.** `resume_on_launch` restores the *prior* play/pause state, it does not force paused. The logic is `load_and_play(...)` (which starts playback) followed by `if !was_playing { pause() }` — so a session that was playing when it closed resumes **playing**. It only lands paused when the persisted status was `paused` or absent. The doc-comment "defaults to paused" refers to the unknown-status fallback, not the common case.

**Fix.** Nothing to fix — this is by design. Just don't assume "resume = always paused" when reasoning about startup audio. If you want a hard always-paused policy, drop the `load_and_play`/`!was_playing` dance and pause unconditionally in [crates/halo-backend/src/commands/player.rs](../crates/halo-backend/src/commands/player.rs) — but confirm the UX change with the user first.

---

## Last.fm scrobbling needs its own threshold — don't reuse `is_played`

**Symptom.** Long tracks scrobble to Last.fm far too early — a 10-minute track scrobbles after ~30 seconds of playback.

**Cause.** Scrobble gating was wired to `halo_core::playback::is_played`, which is the **local play-count** rule: "30 s elapsed **or** 50% played". That's correct for incrementing a play count, but it is *not* Last.fm's rule. Last.fm requires "played for at least **half the track, or 4 minutes, whichever comes first** (min length 30 s)". The 30 s floor in `is_played` makes any track count as scrobbled after 30 s.

**Fix.** Scrobbling uses a separate function, `halo_core::playback::should_scrobble(position_ms, duration_ms)` (50%-or-4min, returns false for tracks under 30 s; falls back to the 4-min cap when duration is unknown). In [player.rs](../crates/halo-backend/src/commands/player.rs) `next_track`/`previous_track`, `is_played` still drives `record_play`/`record_skip`, but the **scrobble** decision goes through `should_scrobble`. Keep the two thresholds distinct — don't "simplify" by collapsing them back into one. Natural track-end (`on_track_finished`) scrobbles unconditionally, which is fine because the track is fully played.

---

## Last.fm app key/secret are persisted; the session only if "Stay signed in" is on

**Symptom.** Scrobbling does nothing and the Settings panel shows the API-key form instead of a Connect button — or, conversely, you expect the saved login to survive a restart and it doesn't.

**Cause.** Two different lifetimes, deliberately split in `lastfm.rs`:
- The **app** credentials (`api_key` / `api_secret`) are entered by the user in Settings → Advanced and **persisted** in `app_state` (`lastfm.api_key` / `lastfm.api_secret`). `LastFmInner::creds()` reads them; until both are set, `lastfm_get_status().configured` is false and the panel shows the credentials form. These survive restarts (entered once by whoever sets up the machine).
- The per-user **session** (`session_key` / `username`) is **memory-only by default** — `load()` purges `lastfm.session_key` / `lastfm.username` on startup, so each person on a shared machine signs into their own account and the login clears on exit. If the user turns on **"Stay signed in on this computer"** (`lastfm.stay_signed_in = "true"`, `lastfm_set_stay_signed_in`), the session is written on connect (or immediately when toggled on while connected) and restored by `load()`. Turning it off, Disconnect, and changing the API key all delete the stored session.
- A restored session can go stale (user revoked Halo at last.fm/settings/applications). Last.fm then answers **error 9 (Invalid session key)**; `invalidate_session` clears it in memory + DB so the panel shows disconnected. It only clears if the session is still the one that failed, so a racing re-auth isn't wiped. Queued scrobbles are kept for the next sign-in.

(History: the key was briefly a compiled-in constant, then reverted on user request — they wanted it entered in the Settings screen, not baked into source. The session was memory-only with no opt-in until 2026-10, when "Stay signed in" was added.)

**Fix.** Nothing to fix — by design. To enable Last.fm: register an app at last.fm/api/account/create and paste the key + shared secret into the Settings → Advanced form (not into source). `lastfm_set_credentials` persists them and clears any stale session. Keep the session opt-in and default-off — persisting it unconditionally would leak one user's scrobbling onto the next user of a shared machine.

---

## Migration files must be wired into `migrations.rs` by hand

**Symptom.** A table defined in `crates/halo-backend/migrations/*.sql` doesn't exist at runtime. Code that writes to it with `let _ = conn.execute(...)` fails silently (this is how the Last.fm offline scrobble queue lost every queued scrobble).

**Cause.** The runner in [crates/halo-backend/src/db/migrations.rs](../crates/halo-backend/src/db/migrations.rs) doesn't scan the directory — each file is `include_str!`'d explicitly behind a `PRAGMA user_version` check. Two files once shared the `007_` prefix (`007_waveform.sql`, `007_lastfm.sql`); only one was wired up, so `lastfm_scrobble_queue` was never created.

**Fix.** Every new `.sql` file gets a unique next number **and** a matching `if version < N` block. The scrobble queue now lives in `009_lastfm_queue.sql` (idempotent `CREATE TABLE IF NOT EXISTS`, safe on any DB).

---

## Queue positions must stay contiguous; bulk queue writes must be one transaction

**Symptom.** "Play All" / "Shuffle Play" on a big library freezes the app for seconds (UI state, media keys and the 250 ms ticker all stall). Or, after a hand-written queue edit, Next/Previous play the wrong track.

**Cause.** Two things, both measured at 50k tracks:
- Row-by-row autocommit inserts cost one WAL commit each — `replace_queue` took **~7.2 s**, all while holding the DB `Mutex`. One transaction + one prepared statement: **~150 ms** (debug build).
- `track_at_index` used to `ORDER BY position LIMIT 1 OFFSET n` with no index (~17 ms per Next). It is now an indexed point lookup `WHERE position = n` (~0.02 ms, `010_queue_position_index.sql`), which is only correct because **every queue writer keeps `position` contiguous 0..n-1** (`replace_queue` numbers from 0, `add_to_queue` appends at MAX+1, `remove_from_queue` deletes + shifts down in one transaction). A gap falls back to the old OFFSET query, so it stays correct, just slower.

**Fix / rule.** Any new queue write (reorder, insert-at, bulk add) must keep positions contiguous and run inside `conn.unchecked_transaction()`. Shuffle state is kept in memory (`ShuffleState`); persist the cursor per move and the full order only when it changes — don't reintroduce a full JSON round-trip per Next. Re-measure with `cargo test -p halo --lib large_queue_timing -- --ignored --nocapture`.

---

## "Open with Halo": three delivery paths, Explorer launches one process per file

**Symptom.** Selecting several files in Windows Explorer → Open plays only the last one. Or double-clicking works on Windows/Linux but not macOS. Or a Linux `.deb` install doesn't offer Halo for audio files.

**Cause.**
- Windows Explorer starts **one process per selected file** (`halo.exe "file"`), each forwarded to the running instance over the single-instance socket ([crates/halo-app/src/single_instance.rs](../crates/halo-app/src/single_instance.rs)) as a separate message. Each replacing the queue means only the last survives.
- macOS never passes files as argv — they arrive as an "open documents" Apple Event. iced/winit exposes no API for it, so **files opened while Halo is already running on macOS are not handled yet** (known gap).
- Linux association comes from `MimeType=` in the `.desktop` file. Our deb template ([crates/halo-app/linux/halo.desktop.hbs](../crates/halo-app/linux/halo.desktop.hbs)) must keep its `MimeType` line. `mimeType` values must match what shared-mime-info detects (e.g. `.ogg` sniffs as `audio/x-vorbis+ogg`, `.wav` is `audio/vnd.wave`), hence the `;`-separated lists in the `fileAssociations` of [crates/halo-app/Cargo.toml](../crates/halo-app/Cargo.toml).
- AppImage cannot register file associations at all (needs AppImageLauncher / appimaged).

**Fix.** All paths funnel into `open_files::enqueue` ([crates/halo-backend/src/open_files.rs](../crates/halo-backend/src/open_files.rs)), which coalesces for 300 ms before opening the batch as one queue. Cold-start argv is handled in `main` via `Backend::handle_launch_files` *instead of* `resume_on_launch`; forwarded launches go through `Backend::handle_forwarded_args`. `PLAYABLE_EXTENSIONS` and the packager `fileAssociations` must list the same formats (only ones rodio can decode — not opus/wma).

---

## Never call `emit_state` (or anything that locks the DB) while holding the DB lock

**Symptom.** The whole app freezes — UI stops updating, media keys and buttons do nothing — after a specific action. Reproduced case: shuffle on, Next past the end of the queue, then Previous.

**Cause.** The DB connection is a `std::sync::Mutex<Connection>`, which is **not re-entrant**: locking it again on the same thread blocks forever. `previous_track` called `emit_state(&app)` inside the block where it still held `conn`; `emit_state` → `build_full_state` locks the DB again → self-deadlock. The 250 ms ticker then blocks on the same lock, so everything stops. (The branch only ran at the start of a shuffle pass, which is why it went unnoticed.)

**Fix / rule.** Inside a `let conn = db.lock()…` scope, only use `&conn`-taking helpers. Call `emit_state`, other command fns, or anything that does `app.state::<Mutex<Connection>>().lock()` / `b.db().lock()` only after the guard is dropped (end the block or `drop(conn)`). Lock order for other state is DB first, then `ShuffleState` / `LastFmState`. MPRIS (`busctl --user call org.mpris.MediaPlayer2.halo /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player Next`) drives the same command paths as the buttons — handy for reproducing.

---

## Windows SMTC needs the main window's HWND — keep the main window alive

**Symptom.** No media-panel integration on Windows, and `media controls init failed` in the log; or SMTC stops reacting after the main window is closed.

**Cause.** souvlaki's Windows backend `expect`s an HWND (it panics without one), and the SMTC session is bound to that window.

**Fix.** `Halo::start_media_controls` ([crates/halo-app/src/app.rs](../crates/halo-app/src/app.rs)) fetches the HWND with `window::run` + `raw-window-handle` *after* the main window has opened and only then calls `Backend::start_media_controls`. Closing to tray and opening the mini player **hide** the main window (`window::set_mode(Hidden)`); never `window::close` it while the app keeps running.

---

## Linux tray icon needs its own GTK main loop

**Symptom.** No tray icon on Linux, or a panic about GTK not being initialised.

**Cause.** `tray-icon` uses libappindicator / GTK, which needs `gtk::init()` and a running `gtk::main()`. winit (iced) doesn't drive GTK.

**Fix.** [crates/halo-app/src/tray.rs](../crates/halo-app/src/tray.rs) builds the icon on a dedicated `halo-tray` thread that calls `gtk::init()` then `gtk::main()`. On Windows/macOS the icon must instead be created on the main thread, which is where iced runs `update` (done on the first `WindowOpened`). Menu / click events reach iced via channels → `tray::events()` subscription.

**Desktop coverage.** libayatana-appindicator talks StatusNotifierItem (SNI) + DBusMenu, and falls back to a legacy XEmbed icon on X11 when no SNI watcher exists:

| Desktop | Tray |
|---|---|
| KDE Plasma, COSMIC, Deepin, Cinnamon (xapp-sn-watcher), XFCE 4.16+ (Status Tray), Budgie 10.9+ | SNI |
| Older XFCE / Budgie, MATE | XEmbed fallback (X11 only) |
| GNOME | none, unless the AppIndicator extension is installed (`gnome-shell-extension-appindicator`; the .deb Suggests it) |

- SNI hosts usually open the menu on **right**-click (left-click on KDE, since appindicator items set `ItemIsMenu`). There is no left-click "Show" event on Linux; use the menu's "Show Halo".
- **No tray, no hiding:** with "minimize to tray", the close button first calls `tray::host_available()`. It checks for a D-Bus `org.kde.StatusNotifierWatcher` with `IsStatusNotifierHostRegistered`, or an X11 `_NET_SYSTEM_TRAY_S<n>` owner, and minimizes instead of hiding when neither exists. Otherwise plain GNOME users would lose the window until they relaunch Halo.
- **Headless testing:** `dbus-launch` fails in the sandbox; use `dbus-run-session -- …`. `stalonetray` provides an XEmbed tray and `gtk-sni-tray-standalone --watcher` an SNI tray; both are on Ubuntu's apt.

---

## Tray actions are lost if `tray::events()` starts before the channel exists

**Symptom.** The tray icon and menu appear, but clicking Play/Pause, Show Halo, Mini player or Quit does nothing.

**Cause.** `tray::events()` takes the receiver out of a static once, when iced starts the subscription — which is *before* `tray::create()` runs (on the main window's first `WindowOpened`). If the channel didn't exist yet, the stream got `None`, idled forever, and iced never rebuilds a subscription whose identity didn't change.

**Fix.** `events()` calls `channel()` before taking the receiver ([crates/halo-app/src/tray.rs](../crates/halo-app/src/tray.rs)). Whatever runs first creates the channel.

---

## Mini player position is stored in physical pixels; iced positions are logical

**Symptom.** The mini player "doesn't open": the main window hides but no mini window is visible (it's off-screen), typically on a scaled (125–200 %) display or after unplugging a monitor.

**Cause.** `mini.pos.x/y` in `app_state` are **physical** pixels — the Tauri build saved them that way and the DB is shared. iced's `window::Position::Specific` and `window::Event::Moved` are **logical**. Reading physical values as logical pushes the window past the screen edge by the scale factor.

**Fix.** [crates/halo-app/src/components/mini.rs](../crates/halo-app/src/components/mini.rs) opens in two steps:
1. Fetch `window::scale_factor(main)`.
2. `commands::mini::open_position_physical` clamps the saved physical position into the nearest monitor's work area (Win32 `MonitorFromPoint` + `GetMonitorInfoW`), and the result is divided by the scale.

Saves multiply by the same scale and clamp again; a window dropped outside the work area is moved back, as the Tauri build did. `transparent: true` is set only on Linux (it lets GTK honour the 30 px height); on Windows a transparent wgpu surface can leave the window blank.

---

## Wayland: no hiding, no always-on-top, no positioning, so Halo runs under XWayland

**Symptom.** On a Wayland session (MX / Debian KDE Plasma, GNOME), opening the mini player leaves the main window on screen, the mini player isn't kept above other windows and ignores its saved position, and close-to-tray doesn't hide.

**Cause.** Wayland gives clients no way to do those things, and winit 0.30's Wayland backend makes `set_visible`, `set_window_level` and `set_outer_position` no-ops. winit picks Wayland whenever `WAYLAND_DISPLAY` is set. (The Tauri build hid windows through GTK, which unmaps the surface on Wayland.)

**Fix.** `prefer_x11()`, first thing in [crates/halo-app/src/main.rs](../crates/halo-app/src/main.rs), removes `WAYLAND_DISPLAY` / `WAYLAND_SOCKET` when `DISPLAY` is set, so winit (and GTK for the tray) use X11 through XWayland, where KWin and Mutter honour unmapping, `_NET_WM_STATE_ABOVE` and window positions. It must run before any thread starts. `HALO_WAYLAND=1` keeps native Wayland, accepting the limits above. Programs Halo launches get the original `WAYLAND_DISPLAY` back through `crate::open_url`; use it instead of `open::that`. Trade-off: on a fractionally scaled GNOME display XWayland windows may look soft.

---

## MPRIS `desktop_entry` must match the installed `.desktop` basename

**Symptom.** GNOME Shell's media widget shows a generic icon for Halo.

**Cause.** The widget resolves the app icon from MPRIS `DesktopEntry`. cargo-packager names the desktop file after the **binary** (`halo.desktop`), whereas Tauri's bundler used the product name.

**Fix.** `desktop_entry: "halo"` in [crates/halo-backend/src/audio/media_controls.rs](../crates/halo-backend/src/audio/media_controls.rs). The window app-id is set to `com.saravix.halo` (`platform_specific.application_id`), matching `StartupWMClass` in the template.

---

## The `.deb` package is `halo-music-player`, not `halo`

**Symptom.** `dpkg -s halo` / `apt remove halo` says the package is not installed, although `/usr/bin/halo` exists. Release #15's `.deb` install test failed on Debian 12 and 13 this way (`dpkg -s halo | grep …` → exit 1).

**Cause.** cargo-packager names the Debian package after `productName` in kebab case ("Halo Music Player" → `halo-music-player`). The binary, the `.deb` file name (`halo_<version>_amd64.deb`) and `halo.desktop` follow the binary name instead.

**Fix.** Use `halo-music-player` for dpkg/apt commands, or read the name from the file (`dpkg-deb -f halo_*.deb Package`), as the release workflow's install test does. Changing `productName` renames the package, so an upgrade would install alongside the old one instead of replacing it.

---

## iced: `renderer.with_layer` escapes the parent's clip

**Symptom.** Text in a virtualised list paints outside the table / scrollable bounds (e.g. a half-scrolled row's title drawn over the border).

**Cause.** A new layer's clip rectangle is absolute; it is not intersected with the clip of the scrollable you're inside.

**Fix.** In custom widgets, draw text with `fill_paragraph(…, clip_bounds)` where `clip_bounds = bounds ∩ viewport` instead of wrapping it in `with_layer`. See `TextLine::draw` in [crates/halo-app/src/widgets/textline.rs](../crates/halo-app/src/widgets/textline.rs).

---

## iced: `Length::FillPortion(0)` is not zero width

**Symptom.** A progress bar at 0% renders completely full.

**Cause.** A zero portion still takes space when it's the only fill sibling with content.

**Fix.** Use `ui::progress(fraction, …)` ([crates/halo-app/src/ui.rs](../crates/halo-app/src/ui.rs)), which special-cases 0% and 100%.

---

## iced: `responsive` closures run more than once

**Symptom.** A cell inside `responsive` renders correctly once, then empty after a resize / re-layout.

**Cause.** iced may call the `responsive` closure on every layout. Moving an element out of a `RefCell` inside it (`take()`) leaves nothing for the next call.

**Fix.** Build everything inside the closure from borrowed data. For grids, `common::vgrid` passes the computed cell width to the item builder instead (used to make album art / avatars square).

---

## Testing the UI headless (Xvfb)

**Symptom.** Under Xvfb the app panics (`libxkbcommon-x11.so could not be loaded`), or keyboard input (xdotool) never reaches the window, or the audio backend fails to start.

**Cause / Fix.**
- Install `libxkbcommon-x11-0` at runtime (winit dlopens it).
- There is no window manager, so X focus isn't given to the window: run `xdotool windowfocus <wid>` before typing.
- No sound card: create `~/.asoundrc` with `pcm.!default { type null }`. The null device consumes audio **much faster than real time**, so tracks end in a second or two — pause playback before testing anything that resets on track change (e.g. the lyrics editor).
- Set `XDG_RUNTIME_DIR` to an existing 0700 directory.
- **With a real window manager (KWin):** run a copy of `kwin_wayland` (the original has `cap_sys_nice`, which a sandbox won't execute) as `kwin_wayland --virtual --xwayland --no-lockscreen --exit-with-session script.sh` inside `unshare -r -n -m` + `dbus-run-session`. In the namespace, mount a tmpfs on `/tmp/.X11-unix` first (KWin refuses a socket dir it doesn't own). `-n` also keeps the abstract single-instance socket away from a running Halo. Keep `XDG_RUNTIME_DIR` short: the Wayland socket path must fit in 108 bytes. The script then sees `DISPLAY` and can use `xdotool`, `xprop` and `xwininfo` (e.g. `Map State`, `_NET_WM_STATE_ABOVE`).

---

## Template

```
## <short title>

**Symptom.** What you observe going wrong.

**Cause.** Why it happens — the underlying mechanism, not just "it's broken".

**Fix.** Concrete remediation, with file links to the relevant code.
```
