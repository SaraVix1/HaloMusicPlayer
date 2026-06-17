# Updating the App Icon

How to replace Halo's app icon and ensure the new icon shows up everywhere on Windows (window, taskbar, executable, installer).

## 1. Generate icon files from a source PNG

Place your source PNG (ideally 1024×1024, transparent background) in the project root and run:

```powershell
npx tauri icon app-icon-source.png
```

This populates [src-tauri/icons/](../../src-tauri/icons/) with every required size:

| Platform | Files |
|---|---|
| Windows | `icon.ico`, `Square*Logo.png` (Store) |
| macOS   | `icon.icns` |
| Linux   | `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png` |
| iOS     | `ios/AppIcon-*` |
| Android | `android/mipmap-*` |

The set referenced by Tauri's bundler is listed under `bundle.icon` in [src-tauri/tauri.conf.json](../../src-tauri/tauri.conf.json).

## 2. Force the binary to re-embed the icon

On Windows the `.ico` is compiled into the executable as a resource by `tauri-build`. Cargo's incremental build won't notice the new `.ico` and will keep the old icon baked in.

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
npm run tauri dev
```

The new icon will show in the taskbar, Alt+Tab, and the executable in File Explorer.

## Troubleshooting

**Taskbar still shows the old icon after running the script.**
If Halo is pinned to the taskbar, the icon is cached inside the `.lnk` shortcut. Right-click the pinned icon → *Unpin from taskbar*, launch the app again, then re-pin it.

**The window's custom title bar doesn't show an icon.**
Expected — `decorations: false` in [tauri.conf.json](../../src-tauri/tauri.conf.json) means there is no native title bar. If you want a logo there, add an `<img>` to [src/components/title-bar.tsx](../../src/components/title-bar.tsx).

**Installer (`.msi` / `.exe`) shows the old icon.**
Run `npm run tauri build` after the cache clean. The bundler embeds `icon.ico` into the installer at build time.
