//! Puts the Cairn logo and version details on cairn.exe, so Explorer, the
//! taskbar, shortcuts, and Settings → Apps show them. Only when building for
//! Windows; macOS and Linux get their icons when the app is packaged.

fn main() {
    println!("cargo:rerun-if-changed=installer/icons/cairn.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("installer/icons/cairn.ico")
            .set("FileDescription", "Cairn")
            .set("ProductName", "Cairn");
        res.compile()
            .expect("the icon could not be added to cairn.exe");
    }
}
