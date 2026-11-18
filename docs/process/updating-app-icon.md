# Updating the App Icon

How to replace Halo's app icon and ensure the new icon shows up everywhere on Windows (window, taskbar, executable, installer).

## 1. Generate icon files from a source PNG

Start from a source PNG (ideally 1024×1024, transparent background) and export the sizes below with any icon tool (e.g. ImageMagick: `magick src.png -resize 128x128 128x128.png`, `magick src.png -define icon:auto-resize=256,128,64,48,32,16 icon.ico`; on macOS `iconutil` for `icon.icns`). Replace the files in [crates/halo-app/assets/icons/](../../crates/halo-app/assets/icons/) :

| Platform | Files |
|---|---|
| Windows | `icon.ico`, `Square*Logo.png` (Store) |
| macOS   | `icon.icns` |
| Linux   | `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png` |

The set used by the installers is listed under `icons` in `[package.metadata.packager]` of [crates/halo-app/Cargo.toml](../../crates/halo-app/Cargo.toml). The window / tray icons are embedded from `128x128.png` and `32x32.png` at compile time.

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
