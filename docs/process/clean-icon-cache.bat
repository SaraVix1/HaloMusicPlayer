@echo off
echo Cleaning Windows icon cache and Cargo build for Halo...
echo.

echo [1/6] Stopping Halo if running...
taskkill /F /IM "Halo Music Player.exe" >nul 2>&1
taskkill /F /IM "Halo.exe" >nul 2>&1
taskkill /F /IM "halo.exe" >nul 2>&1

echo [2/6] Running cargo clean to force icon re-embed...
pushd "%~dp0..\..\src-tauri"
cargo clean -p halo
popd

echo [3/6] Stopping Explorer...
taskkill /F /IM explorer.exe >nul 2>&1

echo [4/6] Deleting icon cache files...
del /A /Q "%LOCALAPPDATA%\IconCache.db" >nul 2>&1
del /A /F /Q "%LOCALAPPDATA%\Microsoft\Windows\Explorer\iconcache_*.db" >nul 2>&1
del /A /F /Q "%LOCALAPPDATA%\Microsoft\Windows\Explorer\thumbcache_*.db" >nul 2>&1

echo [5/6] Refreshing shell icons...
ie4uinit.exe -show >nul 2>&1

echo [6/6] Restarting Explorer...
start explorer.exe

echo.
echo Done. Now run: npm run tauri dev
echo If Halo is pinned to the taskbar, unpin and re-pin it to refresh.
pause
