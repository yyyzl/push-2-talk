fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=src/platform/macos/native.m");
        println!("cargo:rerun-if-changed=src/platform/macos/clipboard.m");
        cc::Build::new()
            .file("src/platform/macos/native.m")
            .file("src/platform/macos/clipboard.m")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .compile("ptt_macos");
        for framework in ["AppKit", "ApplicationServices", "AVFoundation", "Carbon"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    tauri_build::build()
}
