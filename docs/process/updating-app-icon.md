# Updating the App Icon

How to replace Halo's app icon and ensure the new icon shows up everywhere on Windows (window, taskbar, executable, installer).

## 1. Edit the SVG sources and export

The icon is drawn as SVG in [crates/halo-app/assets/icons/src/](../../crates/halo-app/assets/icons/src/):

| Source | Used for |
|---|---|
| `halo.svg` | master: 48 px and up (also the Linux `scalable` icon) |
| `halo-32.svg` | 32 and 40 px, drawn on the 32 px grid |
| `halo-24.svg` | 20 and 24 px, drawn on the 24 px grid |
| `halo-16.svg` | 16 px, drawn on the 16 px grid (blue notes only) |

The small sizes are separate drawings, not shrunk copies of the master: stems and note heads sit on whole pixels so they stay sharp. When you change the master, redraw them to match.

Then export everything:

```bash
python3 scripts/build-icons.py
```

It needs librsvg's Python bindings, pycairo and Pillow (Debian/Ubuntu: `python3-gi gir1.2-rsvg-2.0 python3-cairo python3-pil`) and writes into [crates/halo-app/assets/icons/](../../crates/halo-app/assets/icons/):

| Platform | Files | Used by |
|---|---|---|
| Windows | `icon.ico` (16, 20, 24, 32, 40, 48, 64, 256) | exe resource (build.rs), installers, taskbar / Alt+Tab (`platform::set_window_icon_from_exe`), tray (`Icon::from_resource`) |
| macOS   | `icon.icns` | `.app` / `.dmg` |
| Linux   | `16x16.png` … `512x512.png` | hicolor icons in the `.deb` / `.rpm` / AppImage; `128x128.png` window icon; `64x64.png` tray icon |

Commit the generated files: the build doesn't run the script. Installer icons are listed under `icons` in `[package.metadata.packager]` of [crates/halo-app/Cargo.toml](../../crates/halo-app/Cargo.toml); the `.rpm` lists the same files under `[package.metadata.generate-rpm]` `assets`.

## 2. Force the binary to re-embed the icon

On Windows the `.ico` is compiled into the executable as a resource by [crates/halo-app/build.rs](../../crates/halo-app/build.rs) (`winresource`). Windows caches shell icons aggressively, so the old icon can stick around.

Run the helper script from the project root:

```powershell
.\docs\process\clean-icon-cache.bat
```

It performs:

1. Kills `Halo Music Player.exe` / `halo.exe` if running (the dev binary is `halo.exe`, the installed binary is `Halo Music Player.exe`).
2. `cargo clean -p halo` — forces a full rebuild so the new `.ico` gets re-embedded.
3. Stops Explorer.
4. Deletes Windows icon caches:
   - `%LOCALAPPDATA%\IconCache.db`
   - `%LOCALAPPDATA%\Microsoft\Windows\Explorer\iconcache_*.db`
   - `%LOCALAPPDATA%\Microsoft\Windows\Explorer\thumbcache_*.db`
5. Runs `ie4uinit.exe -show` to refresh shell icons.
6. Restarts Explorer.

## 3. Rebuild and run

```powershell
cargo run -p halo
```

The new icon will show in the taskbar, Alt+Tab, and the executable in File Explorer.

## Troubleshooting

**Taskbar still shows the old icon after running the script.**
If Halo is pinned to the taskbar, the icon is cached inside the `.lnk` shortcut. Right-click the pinned icon → *Unpin from taskbar*, launch the app again, then re-pin it.

**The window's custom title bar doesn't show an icon.**
Expected — the main window is undecorated (`decorations: false` in `main_window_settings`, [crates/halo-app/src/app.rs](../../crates/halo-app/src/app.rs)), so there is no native title bar. If you want a logo there, add an image to [crates/halo-app/src/components/title_bar.rs](../../crates/halo-app/src/components/title_bar.rs).

**Installer (`.msi` / `.exe`) shows the old icon.**
Run `cargo build --release -p halo && cargo packager --release -p halo` after the cache clean. The packager embeds `icon.ico` into the installer at build time.
