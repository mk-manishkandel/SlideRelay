fn main() {
    // macOS: ScreenCaptureKit's Swift bridge needs the system Swift runtime
    // (/usr/lib/swift, part of macOS 12+). Link-args from dependencies don't
    // reach our binaries, so add the search path here for the app and tests.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
    tauri_build::build()
}
