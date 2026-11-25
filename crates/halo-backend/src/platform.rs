//! Process-level OS setup that must run before any window is created.

#[cfg(windows)]
const AUMID: &str = "Halo";

/// Windows: register the AppUserModelID so the SMTC media panel shows "Halo"
/// instead of "Unknown app". No-op elsewhere.
#[cfg(windows)]
pub fn set_app_user_model_id() {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    // Windows SMTC looks up the app's display name by matching this AUMID against
    // a Start Menu shortcut whose System.AppUserModel.ID property is the same
    // string. Without that shortcut the panel shows "Unknown app", so we install
    // one ourselves (Spotify, Discord, VS Code do the same).
    if let Err(e) = ensure_aumid_shortcut() {
        eprintln!("aumid shortcut install failed: {e}");
    }
    let aumid = HSTRING::from(AUMID);
    let _ = unsafe { SetCurrentProcessExplicitAppUserModelID(&aumid) };
}

#[cfg(windows)]
fn ensure_aumid_shortcut() -> Result<(), String> {
    use windows::core::{Interface, HSTRING, PROPVARIANT};
    use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, COINIT_APARTMENTTHREADED,
        CLSCTX_INPROC_SERVER,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    let appdata = std::env::var("APPDATA").map_err(|e| format!("APPDATA: {e}"))?;
    let shortcut_dir =
        std::path::PathBuf::from(&appdata).join(r"Microsoft\Windows\Start Menu\Programs");
    std::fs::create_dir_all(&shortcut_dir).map_err(|e| format!("create_dir_all: {e}"))?;
    let shortcut_path = shortcut_dir.join(format!("{AUMID}.lnk"));

    let exe_path = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "exe has no parent dir".to_string())?;

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("CoCreateInstance(ShellLink): {e}"))?;
        link.SetPath(&HSTRING::from(exe_path.as_os_str()))
            .map_err(|e| format!("SetPath: {e}"))?;
        link.SetWorkingDirectory(&HSTRING::from(exe_dir.as_os_str()))
            .map_err(|e| format!("SetWorkingDirectory: {e}"))?;

        let store: IPropertyStore = link.cast().map_err(|e| format!("cast IPropertyStore: {e}"))?;
        let value: PROPVARIANT = PROPVARIANT::from(AUMID);
        store
            .SetValue(&PKEY_AppUserModel_ID, &value)
            .map_err(|e| format!("SetValue(AUMID): {e}"))?;
        store.Commit().map_err(|e| format!("Commit: {e}"))?;

        let persist: IPersistFile = link.cast().map_err(|e| format!("cast IPersistFile: {e}"))?;
        persist
            .Save(&HSTRING::from(shortcut_path.as_os_str()), true)
            .map_err(|e| format!("Save: {e}"))?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_app_user_model_id() {}

/// Resource ID winresource gives the exe icon (`set_icon` in build.rs).
#[cfg(windows)]
const EXE_ICON_ID: u16 = 1;

/// Windows: point the window's big (taskbar, Alt+Tab) and small icons at
/// the exe's multi-size `.ico`, loaded at the exact sizes for the window's
/// DPI. A single RGBA icon from iced is rescaled by Windows with poor
/// filtering; this lets it pick the hand-tuned 16/24/32 px entries.
#[cfg(windows)]
pub fn set_window_icon_from_exe(hwnd: *mut std::ffi::c_void) {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
    use windows::Win32::UI::WindowsAndMessaging::{
        SendMessageW, ICON_BIG, ICON_SMALL, SM_CXICON, SM_CXSMICON, WM_SETICON,
    };
    let hwnd = HWND(hwnd);
    // SAFETY: plain Win32 calls on a live window handle owned by this process.
    unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96);
        for (which, metric) in [(ICON_BIG, SM_CXICON), (ICON_SMALL, SM_CXSMICON)] {
            let size = GetSystemMetricsForDpi(metric, dpi);
            if let Some(icon) = load_exe_icon(size) {
                SendMessageW(hwnd, WM_SETICON, WPARAM(which as usize), LPARAM(icon as isize));
            }
        }
    }
}

/// Windows: tray icon size in physical pixels (the small-icon metric at the
/// system DPI: 16 at 100 %, 24 at 150 %, 32 at 200 %).
#[cfg(windows)]
pub fn small_icon_size() -> u32 {
    use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi};
    use windows::Win32::UI::WindowsAndMessaging::SM_CXSMICON;
    // SAFETY: plain Win32 calls with no pointer arguments.
    let px = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem().max(96)) };
    if px > 0 { px as u32 } else { 16 }
}

/// Load the exe icon at `size` px. Windows picks the closest entry in the
/// `.ico` (and only scales when no entry is that size). The HICON is shared
/// for the life of the process, so it is never destroyed.
#[cfg(windows)]
unsafe fn load_exe_icon(size: i32) -> Option<*mut std::ffi::c_void> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HINSTANCE;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{LoadImageW, IMAGE_ICON, LR_DEFAULTCOLOR};
    let module = GetModuleHandleW(PCWSTR::null()).ok()?;
    // MAKEINTRESOURCEW(EXE_ICON_ID)
    let name = PCWSTR(EXE_ICON_ID as usize as *const u16);
    let handle =
        LoadImageW(HINSTANCE(module.0), name, IMAGE_ICON, size, size, LR_DEFAULTCOLOR).ok()?;
    Some(handle.0)
}

/// The current hour (0–23) in the user's local time zone, e.g. for the
/// Home page greeting. `None` if the platform can't tell.
#[cfg(unix)]
pub fn local_hour() -> Option<u8> {
    // SAFETY: `time(NULL)` and `localtime_r` with a valid out-pointer to a
    // zeroed `tm`; localtime_r is the thread-safe variant.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut tm).is_null() {
            return None;
        }
        u8::try_from(tm.tm_hour).ok()
    }
}

#[cfg(windows)]
pub fn local_hour() -> Option<u8> {
    // SAFETY: no arguments; returns the local time by value.
    let now = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    u8::try_from(now.wHour).ok()
}

#[cfg(not(any(unix, windows)))]
pub fn local_hour() -> Option<u8> {
    None
}
