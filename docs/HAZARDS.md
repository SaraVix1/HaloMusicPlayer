# Hazards

Non-obvious traps in this codebase. When you hit a footgun that took non-trivial time to diagnose, add it here so the next person (or future you) doesn't pay the same cost.

Each entry: **symptom → cause → fix**.

---

## Tauri: don't build secondary webviews on demand

**Symptom.** A `WebviewWindowBuilder` window opens, but the content is blank white, no console errors, and DevTools (F12) won't open on it.

**Cause.** Building a Tauri v2 webview lazily — inside an IPC command, after the app is fully running — can leave the webview in a half-initialized state on Windows. The OS window appears but the WebView2 process doesn't fully attach.

**Fix.** Build secondary windows during `setup()` with `.visible(false)`, then just `show()` / `hide()` them on demand. See [src-tauri/src/commands/window.rs](../src-tauri/src/commands/window.rs) (`build_mini_window`) and how it's called from [src-tauri/src/lib.rs](../src-tauri/src/lib.rs) `setup()`.

---

## MSVC link.exe shadowed by Git's link.exe

**Symptom.** `cargo build` / `cargo check` fails with cryptic linker errors, or links the wrong binary on Windows.

**Cause.** Git for Windows ships its own `link.exe` (a symlink utility). If Git's `usr\bin` is on `PATH` before Visual Studio's MSVC `link.exe`, Cargo invokes the wrong one.

**Fix.** We sidestep this entirely by configuring `linker = "rust-lld"` in [src-tauri/.cargo/config.toml](../src-tauri/.cargo/config.toml) — rust-lld is bundled with rustup and resolved via the toolchain, not PATH, so Git's `link.exe` can't interfere. Don't remove that config without a reason. If you do need MSVC `link.exe`, install Visual Studio's **"Desktop development with C++"** workload and use the **Developer PowerShell for VS** (not a plain shell).

---

## Cargo won't re-embed a new `icon.ico`

**Symptom.** You replaced `src-tauri/icons/icon.ico`, ran `npm run tauri dev`, and the old icon is still baked into the executable / shown in the taskbar.

**Cause.** `tauri-build` embeds `icon.ico` as a Windows resource at compile time. Cargo's incremental build doesn't track the `.ico` as an input, so it skips the resource compile.

**Fix.** `cargo clean -p halo` to force a full rebuild. The helper script [docs/process/clean-icon-cache.bat](process/clean-icon-cache.bat) does this plus flushes the Windows shell icon cache. See [docs/process/updating-app-icon.md](process/updating-app-icon.md).

---

## shadcn/ui here uses `@base-ui/react`, not `@radix-ui`

**Symptom.** You copy a shadcn example from the public docs and the props don't match — `delayDuration`, `asChild`, etc. produce TypeScript errors or no-op at runtime.

**Cause.** This project uses the newer shadcn/ui flavor that targets `@base-ui/react`. The public docs default to the `@radix-ui` flavor.

**Fix.** Match the API of components already in [src/components/ui/](../src/components/ui/). Common deltas:
- `TooltipProvider` takes `delay`, not `delayDuration`
- `TooltipTrigger` renders a native `<button>` directly — no `asChild` prop

---

## SMTC shows "Unknown app" without an AUMID shortcut

**Symptom.** Windows Media Controls / SMTC panel shows "Unknown app" instead of "Halo" when something is playing.

**Cause.** Windows SMTC looks up the display name by matching the process's AppUserModel.ID against a Start Menu shortcut whose `System.AppUserModel.ID` property matches the same string. No shortcut → no name.

**Fix.** `set_app_user_model_id()` in [src-tauri/src/lib.rs](../src-tauri/src/lib.rs) installs the shortcut itself (the same approach Spotify / Discord / VS Code use). Don't remove it.

---

## `:hover` / `mouseenter` is sticky when a hidden Tauri window is re-shown

**Symptom.** A secondary Tauri window (e.g. mini player) is `hide()`'d, then `show()`'d again. On second open it renders as if the cursor were already hovering — CSS `:hover` rules apply, `onMouseEnter` fires — even though the user hasn't moved the mouse since show.

**Cause.** Hover state is computed from the OS's reported cursor position relative to the window. When the window becomes visible at coordinates where the cursor already is, WebView2 reports the cursor as being inside the window on first paint. Synthetic `mouseenter` may also fire even though there was no boundary-crossing motion. `visibilitychange` resetting state isn't enough either — the synthetic enter races the reset.

**Fix.** Don't trust `:hover` / `mouseenter` to mean "the user is interacting." Gate the hovered state on **accumulated mouse movement** inside the element via `onMouseMove`: sum `Math.hypot(dx, dy)` between successive points and only flip to `hovered = true` past a small threshold (e.g. 8 px). Synthetic events at a static position contribute zero distance, so the controls stay hidden until the user actually moves the mouse. Reset the accumulator on `mouseleave` and on `visibilitychange` away from "visible". See [src/components/mini-player.tsx](../src/components/mini-player.tsx).

---

## Linux/WebKitGTK: `mouseleave` doesn't fire exiting top/left/right of the mini player

**Symptom.** The mini player's control overlay (play/pause, seek bar) shows on hover, correctly hides when the cursor exits through the **bottom** edge, but stays stuck visible when the cursor exits through the **top, left, or right** edge. Windows doesn't have this problem.

**Cause.** Confirmed via temporary `console.log` instrumentation on every `mousemove` (the mini window has `.devtools(true)`, so DevTools is reachable by right-click): `mousemove` events genuinely **stop** arriving in every direction once the cursor leaves — so it isn't a "leave event never fires because the cursor never truly left" situation. The leave-edge case is specifically that neither `onMouseLeave` (element bounding-box) nor `document`-level `mouseout` with `relatedTarget === null` fire on this WebKitGTK build for the top/left/right transitions, even though the bottom transition fires a normal `mouseleave` correctly. This looks like a WebKitGTK pointer-leave delivery gap specific to certain edges, not a CSS or layout bug — exact compositor mechanism unconfirmed.

**Fix.** Don't depend on any leave/exit event for the top/left/right case. While `hovered` is true, run a `window`-level `mousemove` listener that resets a "last moved at" timestamp, polled every 150 ms; if no movement arrives for `IDLE_MS` (600 ms), assume the cursor left and revert to the title view. This is event-independent — it only requires `mousemove` to *stop*, which is confirmed to happen reliably in every direction. The real `mouseleave`/`mouseout` paths are kept as fast-paths (instant reset, still used by Windows and the bottom edge on Linux); the idle-timeout is the universal fallback. See the three stacked `useEffect`s around `hovered` in [src/components/mini-player.tsx](../src/components/mini-player.tsx).

---

## Windows 11 rounds borderless window corners even with `decorations: false`

**Symptom.** A Tauri window with `.decorations(false)` still shows ~6 px rounded corners on Windows 11, no matter what CSS the page applies — `border-radius: 0` on `html/body/root` has no effect, because the corners are clipped by the OS compositor outside the webview.

**Cause.** Windows 11's DWM applies its default `DWMWCP_DEFAULT` (rounded) corner preference to top-level windows. CSS can only style what's inside the webview; the window outline is the OS's.

**Fix.** Call `DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &DWMWCP_DONOTROUND, 4)` after the window is built. See `apply_sharp_corners` in [src-tauri/src/commands/window.rs](../src-tauri/src/commands/window.rs). Requires the `Win32_Graphics_Dwm` feature on the `windows` crate.

---

## Tauri's `window.hwnd()` HWND won't compile against direct `windows` crate calls

**Symptom.** You call a Win32 API like `DwmSetWindowAttribute(window.hwnd()?, …)` and get a cryptic compile error: "there are multiple different versions of crate `windows` in the dependency graph", with mentions of `windows_core::Param<HWND>` and `core::r#type::TypeKind` not being implemented.

**Cause.** Tauri internally depends on a different version of the `windows` crate than the one we declare in `Cargo.toml`. `window.hwnd()` returns an HWND from Tauri's version; Win32 functions we call expect HWND from our version. The two types are nominally the same but Rust treats them as distinct, and no `Param<HWND>` impl bridges them.

**Fix.** Convert through the underlying raw pointer:

```rust
let raw = window.hwnd().ok()?;
let hwnd = windows::Win32::Foundation::HWND(raw.0 as *mut core::ffi::c_void);
```

`HWND.0` is a `*mut c_void` in both versions, so this round-trip is sound. See `apply_sharp_corners` in [src-tauri/src/commands/window.rs](../src-tauri/src/commands/window.rs).

---

## VirtualGrid renders nothing when loading/empty states use a separate return branch

**Symptom.** A component using `VirtualGrid` shows nothing — not even the loading text or empty state — after data loads. `cols` stays 0. Items never appear.

**Cause.** `useLayoutEffect` measures the scroll container and attaches a `ResizeObserver` to `parentRef.current`. If loading/empty states are returned from an *early `return`* that doesn't include the `ref={parentRef}` div, the effect fires with `el = null`, bails out, and the `ResizeObserver` never attaches. When loading finishes and the component re-renders to the main branch (which does have the ref), `useLayoutEffect` does **not** re-run because its deps (`[minItemWidth, gap, px]`) haven't changed — so `cols` stays 0 and the grid never renders.

**Fix.** Always render the same `ref`'d div regardless of state. Put loading/empty content *inside* that div as conditional children, never as early returns with a different root element. See [src/components/virtual-grid.tsx](../src/components/virtual-grid.tsx).

---

## SQLite GROUP_CONCAT: outer query cannot reference an inner alias by table prefix

**Symptom.** A query like `SELECT GROUP_CONCAT(a.name, '||') FROM (SELECT a.name FROM … JOIN artists a …)` fails with `no such column: a.name`.

**Cause.** The outer `SELECT` sees only the column *name* produced by the subquery, not the original table alias `a`. The alias `a` is scoped to the inner query and invisible outside it.

**Fix.** Use the bare column name in the outer GROUP_CONCAT: `SELECT GROUP_CONCAT(name, '||') FROM (SELECT a.name FROM …)`. The derived column is named `name`, not `a.name`. See [src-tauri/src/commands/library.rs](../src-tauri/src/commands/library.rs).

---

## Cargo ignores `[profile.*]` in workspace member packages

**Symptom.** A build prints `warning: profiles for the non root package will be ignored, specify profiles at the workspace root`, and dev-build tuning you put in `src-tauri/Cargo.toml` (faster incremental settings, debug level, etc.) silently has no effect.

**Cause.** In a Cargo workspace, profile settings are only honored in the **root** manifest. A `[profile.dev]` (or any `[profile.*]`) block inside a member package is parsed but ignored — so the tuning appears configured while doing nothing.

**Fix.** Put all `[profile.*]` blocks in the root [Cargo.toml](../Cargo.toml), not in [src-tauri/Cargo.toml](../src-tauri/Cargo.toml) or any `crates/*` member. There's one workspace `target/` dir, so one set of profiles governs everything.

---

## rodio output stays on the startup device; it does not follow the OS default

**Symptom.** You switch the Windows default playback device (headphones → speakers, BT, etc.) while a track is playing. The OS default changes, but Halo keeps sending audio to the *original* device.

**Cause.** `OutputStream::try_default()` opens whatever device is default **at that moment** and binds the stream to it for life. rodio/cpal never migrate an open stream when the system default changes — there's no automatic follow.

**Fix.** Detect the change and rebuild the stream. The 250 ms ticker in [src-tauri/src/lib.rs](../src-tauri/src/lib.rs) polls `get_current_device_name()` (~1 s cadence) and, on a real change, calls `PlayerHandle::rebuild_output()`. That sends `Command::RebuildOutput` to the audio worker, which reopens `OutputStream::try_default()` on the new device, **recomputes the sample rate** (the new device may differ), re-decodes the current track from `current_path`, and seeks back to the saved position — preserving paused state and volume. Skip the very first detection at startup (`last_device` empty) since the stream already opened on the correct default. See `rebuild_output` in [src-tauri/src/audio/mod.rs](../src-tauri/src/audio/mod.rs).

---

## `notify` watcher stops silently if its handle is dropped

**Symptom.** Folder watching "works" right after toggling it on, but stops firing rescans later — or never fires at all if the watcher was created inside a function and not stored anywhere. No error, no panic; filesystem changes are simply ignored.

**Cause.** `notify`'s `RecommendedWatcher` watches only as long as the handle is alive. The OS subscription is torn down in its `Drop` impl. A `let watcher = notify::recommended_watcher(...)` that goes out of scope unsubscribes immediately.

**Fix.** Keep the watcher handle in long-lived state. We store it in `FolderWatcher.watcher: Mutex<Option<RecommendedWatcher>>` (Tauri-managed). Setting that `Option` to `None` is also how we **stop** watching when the toggle is turned off — dropping the handle is the intended off-switch. `reconfigure()` rebuilds the handle from scratch on every change (startup, toggle, folder add/remove) rather than mutating an existing one. See [src-tauri/src/watcher.rs](../src-tauri/src/watcher.rs).

---

## Resume-on-launch defaults to paused only when prior status is unknown

**Symptom.** You expect resume-on-launch to always come up paused (a safe default), but Halo sometimes opens already **playing** audio on launch.

**Cause.** `resume_on_launch` restores the *prior* play/pause state, it does not force paused. The logic is `load_and_play(...)` (which starts playback) followed by `if !was_playing { pause() }` — so a session that was playing when it closed resumes **playing**. It only lands paused when the persisted status was `paused` or absent. The doc-comment "defaults to paused" refers to the unknown-status fallback, not the common case.

**Fix.** Nothing to fix — this is by design. Just don't assume "resume = always paused" when reasoning about startup audio. If you want a hard always-paused policy, drop the `load_and_play`/`!was_playing` dance and pause unconditionally in [src-tauri/src/commands/player.rs](../src-tauri/src/commands/player.rs) — but confirm the UX change with the user first.

---

## Last.fm scrobbling needs its own threshold — don't reuse `is_played`

**Symptom.** Long tracks scrobble to Last.fm far too early — a 10-minute track scrobbles after ~30 seconds of playback.

**Cause.** Scrobble gating was wired to `halo_core::playback::is_played`, which is the **local play-count** rule: "30 s elapsed **or** 50% played". That's correct for incrementing a play count, but it is *not* Last.fm's rule. Last.fm requires "played for at least **half the track, or 4 minutes, whichever comes first** (min length 30 s)". The 30 s floor in `is_played` makes any track count as scrobbled after 30 s.

**Fix.** Scrobbling uses a separate function, `halo_core::playback::should_scrobble(position_ms, duration_ms)` (50%-or-4min, returns false for tracks under 30 s; falls back to the 4-min cap when duration is unknown). In [player.rs](../src-tauri/src/commands/player.rs) `next_track`/`previous_track`, `is_played` still drives `record_play`/`record_skip`, but the **scrobble** decision goes through `should_scrobble`. Keep the two thresholds distinct — don't "simplify" by collapsing them back into one. Natural track-end (`on_track_finished`) scrobbles unconditionally, which is fine because the track is fully played.

---

## Last.fm app key/secret are user-entered in Settings and persisted; session is not

**Symptom.** Scrobbling does nothing and the Settings panel shows the API-key form instead of a Connect button — or, conversely, you expect the saved login to survive a restart and it doesn't.

**Cause.** Two different lifetimes, deliberately split in `lastfm.rs`:
- The **app** credentials (`api_key` / `api_secret`) are entered by the user in Settings → Advanced and **persisted** in `app_state` (`lastfm.api_key` / `lastfm.api_secret`). `LastFmInner::creds()` reads them; until both are set, `lastfm_get_status().configured` is false and the panel shows the credentials form. These survive restarts (entered once by whoever sets up the machine).
- The per-user **session** (`session_key` / `username`) is **memory-only** — `load()` purges `lastfm.session_key` / `lastfm.username` on startup and never writes them, so each person on a shared machine signs into their own account and the login clears on exit.

(History: the key was briefly a compiled-in constant, then reverted on user request — they wanted it entered in the Settings screen, not baked into source.)

**Fix.** Nothing to fix — by design. To enable Last.fm: register an app at last.fm/api/account/create and paste the key + shared secret into the Settings → Advanced form (not into source). `lastfm_set_credentials` persists them and clears any stale session. Don't "simplify" by persisting the session key — that would leak one user's scrobbling onto the next user of a shared machine.

---

## Linux/Wayland: app icon not showing in GNOME dock

**Symptom.** The app runs fine but shows a blank or generic icon in the GNOME taskbar/dock. `set_icon()` calls have no effect.

**Cause.** On Wayland, GNOME Shell ignores `_NET_WM_ICON` (`set_icon()` is X11-only). Instead it reads the xdg-toplevel **app-id** set via the Wayland protocol, looks for `<app-id>.desktop` in XDG application dirs, and loads the icon from that file. Two sub-problems conspire:

1. **Wrong app-id.** tao (Tauri's windowing layer) derives the Wayland app-id from `g_get_prgname()`, which defaults to the binary name (`"halo"`), not the Tauri identifier (`"com.saravix.halo"`). GNOME Shell then looks for `halo.desktop`, which doesn't exist.

2. **GLib silently rejects `.desktop` files with a non-existent `Exec=` path.** `g_desktop_app_info_new_from_filename()` returns NULL when the declared binary doesn't exist, making the entire file invisible to GNOME Shell — no error is logged. In a Cargo workspace the binary lands in the **workspace root** `target/debug/halo`, *not* `src-tauri/target/debug/halo`.

**Fix.** Two permanent changes:

1. In [src-tauri/src/main.rs](../src-tauri/src/main.rs), call `g_set_prgname("com.saravix.halo")` via FFI *before* `halo_lib::run()` on Linux so tao picks up the right app-id:
   ```rust
   #[cfg(target_os = "linux")]
   unsafe {
       extern "C" { fn g_set_prgname(prgname: *const std::os::raw::c_char); }
       g_set_prgname(b"com.saravix.halo\0".as_ptr() as _);
   }
   halo_lib::run()
   ```

2. Install a dev-only `.desktop` file at `~/.local/share/applications/com.saravix.halo.desktop` with `Exec=` pointing to the **actual** compiled binary:
   ```ini
   [Desktop Entry]
   Name=Halo Music Player
   Exec=/home/sara/Work/Rust/HaloMusicPlayer/target/debug/halo
   Icon=/home/sara/.local/share/icons/hicolor/128x128/apps/com.saravix.halo.png
   Type=Application
   Categories=AudioVideo;Audio;Music;Player;
   StartupWMClass=com.saravix.halo
   ```
   Copy the icon from `src-tauri/icons/128x128.png` to that icon path. Run `update-desktop-database ~/.local/share/applications/` after any change.

To diagnose: run `gtk-launch com.saravix.halo`; if it says "no such application", GLib rejected the `.desktop` file — almost always because `Exec=` points to a missing binary. Verify with `WAYLAND_DEBUG=1` that the app emits `set_app_id("com.saravix.halo")`.

For **production builds** (`tauri build`), Tauri auto-generates and installs the `.desktop` file with the correct installed path — the manual dev file is not needed.

---

## Linux: mini player broken — window too tall and main window doesn't hide

Three interlocking problems on Linux/Wayland. All fixed in [src-tauri/src/commands/window.rs](../src-tauri/src/commands/window.rs), [src/components/mini-player.tsx](../src/components/mini-player.tsx), and [index.html](../index.html).

### 0 — `set_always_on_top` races the compositor window-map on Linux/Wayland

**Symptom.** The mini player appears but sits behind other windows — not always-on-top despite the setting being enabled. No errors are returned by `set_always_on_top`.

**Cause.** On GNOME Wayland, the compositor maps windows **asynchronously** after `show()`. Calling `set_always_on_top` immediately after `show()` races the window-map event and the compositor discards the hint. Confirmed the mechanism itself is sound: manually toggling "Always on Top" from the window's title-bar right-click menu (a GNOME Shell / Mutter feature, not ours) works instantly and persists — proving `gtk_window_set_keep_above()` does work on this GNOME/Mutter version once the window is fully mapped. A single re-apply at a fixed delay (e.g. 250 ms) was not reliable enough under system load.

**Fix.** Call `set_always_on_top` immediately after `show()`, then retry it repeatedly from a background thread at increasing gaps (100/250/500/1000/2000 ms cumulative) for about 2 seconds (Linux-only via `#[cfg(target_os = "linux")]`). One of the retries is guaranteed to land after the compositor finishes mapping the window, at which point the hint sticks exactly like the manual toggle. See `open_mini_player` in [src-tauri/src/commands/window.rs](../src-tauri/src/commands/window.rs).

**What didn't work.**
- `gdk_set_allowed_backends("x11")` — forces GTK to use XWayland; panics ("Failed to initialize GTK") when `DISPLAY` auth isn't accessible in the launch environment (e.g. when spawned via `npm run tauri dev`).
- `gdk_set_allowed_backends("x11,wayland")` — falls back to Wayland when x11 init fails, so AOT still doesn't work.
- Guarding either call with `GDK_BACKEND.is_none()` — Wayland sessions always export `GDK_BACKEND=wayland`, so the guard skips the call on every login.

### 1 — Main window stays visible when mini player opens

**Symptom.** Opening the mini player shows it floating over the main Halo window instead of replacing it.

**Cause.** `open_mini_player` used `?` on `set_always_on_top` and `set_focus`. On Linux/Wayland, both are routinely rejected by the compositor (focus stealing is blocked; "always on top" hints are not universally supported). When either returned `Err`, Rust's `?` exited the function early — **before the `main.hide()` call at the end**. The mini player was already visible (`.show()` had succeeded) but the main window never hid.

**Fix.** Change both to `.ok()`. They are best-effort on every platform; `.show()` remains a hard error since failure there means no mini player at all. Also call `set_always_on_top` *after* `show()` + `unminimize()` — on GTK the keep_above hint only takes effect on a mapped (visible) window.

### 2 — Window height ignores the 30 px constraint (GTK compositor minimum)

**Symptom.** The mini player content is correctly 30 px but sits inside a much taller dark-background window — the window height is 120–200 px depending on the compositor.

**Cause.** Wayland compositors enforce a minimum window height that overrides GTK's own geometry hints (`min_inner_size` / `max_inner_size` / `resizable(false)`). The `inner_size(280, 30)` builder call and a post-build `set_size(LogicalSize::new(280, 30))` are both silently overridden by the compositor after the window is mapped.

**Fix (workaround — can't defeat the compositor).** Three layers:
1. `.transparent(true)` on the builder — forces an ARGB composited surface, which removes the opaque GTK chrome that contributes to minimum height.
2. Post-build `#[cfg(target_os = "linux")] window.set_size(LogicalSize::new(MINI_W, MINI_H))` — belt-and-suspenders attempt.
3. Initialization script sets `document.documentElement.style.background = 'transparent'` and `document.body.style.background = 'transparent'` — so the compositor-imposed extra height below the 30 px content strip is transparent (invisible + click-through on Wayland) rather than a solid dark rectangle.

On Windows/macOS, `transparent(true)` is harmless: the window is exactly 30 px, so there is no extra space for transparency to expose, and the MiniPlayer root's `bg-background` (opaque) covers the entire window.

### 3 — Content height driven by font metrics, not the viewport

**Symptom.** The mini player content area is visibly taller than 30 px even before the compositor minimum is hit — on Linux, default font metrics make the two-line text layout slightly taller than on Windows.

**Cause.** The mini player root div used `h-full` (= `height: 100%`). On WebKitGTK, `100%` resolves against the parent's computed height; because `html`/`body`/`#root` all had `height: auto`, it effectively meant "as tall as the content's natural height". Different font metrics on Linux made the natural height slightly > 30 px, causing WebKitGTK to report a larger preferred size to GTK and GTK to grow the window accordingly.

**Fix.** Changed `h-full` → `h-[30px]` on the mini player root div in [src/components/mini-player.tsx](../src/components/mini-player.tsx). This pins the WebKit layout to exactly 30 px regardless of font metrics or the viewport height chain. Also added `height: 100%; overflow: hidden` to `html`, `body`, and `#root` in [index.html](../index.html) so the main window's `h-full` chain resolves correctly on all platforms.

### Also fixed: default position on Linux

`default_mini_position` returned `None` on non-Windows, landing the mini player at `center()`. It now computes bottom-right using the primary monitor's physical size with a 56 logical-px bottom margin to clear typical taskbars/docks.

---

## Template

```
## <short title>

**Symptom.** What you observe going wrong.

**Cause.** Why it happens — the underlying mechanism, not just "it's broken".

**Fix.** Concrete remediation, with file links to the relevant code.
```
