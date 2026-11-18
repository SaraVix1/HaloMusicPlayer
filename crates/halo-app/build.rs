//! Embeds the app icon into the Windows executable (taskbar / Explorer).

fn main() {
    println!("cargo:rerun-if-changed=assets/icons/icon.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/icon.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to embed icon: {e}");
        }
    }
}
