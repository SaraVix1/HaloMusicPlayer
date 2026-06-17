use std::ffi::c_void;

/// OS-specific configuration needed to create media controls.
#[derive(Debug)]
pub struct PlatformConfig<'a> {
    /// The name to be displayed to the user. (*Required on Linux*)
    pub display_name: &'a str,
    /// Should follow [the D-Bus spec](https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-names-bus). (*Required on Linux*)
    pub dbus_name: &'a str,
    /// The basename (no `.desktop` suffix) of the installed `.desktop` file,
    /// published as MPRIS's `DesktopEntry` property. GNOME Shell's media
    /// widget uses this — not `Identity` — to resolve the app icon. Pass ""
    /// if no matching `.desktop` file is installed. (Linux only)
    pub desktop_entry: &'a str,
    /// An HWND. (*Required on Windows*)
    pub hwnd: Option<*mut c_void>,
}
