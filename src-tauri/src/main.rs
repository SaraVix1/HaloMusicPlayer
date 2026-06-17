// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // On Linux/Wayland, tao derives the xdg-toplevel app-id from g_get_prgname() when no
    // GApplication is in use. Override it to our identifier before GTK initializes so that
    // GNOME Shell matches the window to com.saravix.halo.desktop and shows the correct icon.
    #[cfg(target_os = "linux")]
    unsafe {
        extern "C" {
            fn g_set_prgname(prgname: *const std::os::raw::c_char);
        }
        g_set_prgname(b"com.saravix.halo\0".as_ptr() as _);
    }

    halo_lib::run()
}
