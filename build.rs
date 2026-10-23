fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icons/BrowserIcon.ico");
    // Explorer, the taskbar and shortcuts take the executable's icon from this resource.
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/icons/BrowserIcon.ico")
            .compile()
            .expect("embed the Windows icon resource");
    }
}
