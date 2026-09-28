// Links the dependencies of the statically-built FFmpeg (see src/media.rs).
//
// ffmpeg-sys-next finds FFmpeg itself through FFMPEG_DIR (set in the
// workspace .cargo/config.toml to our vcpkg_installed/<triplet>) and links
// the av* libraries, but not what those static libraries depend on. The list
// below is FFmpeg's own `Libs`/`Libs.private` from vcpkg's generated
// pkgconfig files for our feature set (dav1d, openh264, QSV via libmfx, plus
// the Windows system libraries avdevice/avformat/avutil use).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }
    let Ok(ffmpeg_dir) = std::env::var("FFMPEG_DIR") else {
        println!("cargo:warning=FFMPEG_DIR is not set; FFmpeg's static dependencies won't be linked");
        return;
    };
    println!("cargo:rustc-link-search=native={}", std::path::Path::new(&ffmpeg_dir).join("lib").display());

    for lib in ["dav1d", "openh264", "libmfx"] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    for lib in [
        "mfuuid", "ole32", "oleaut32", "strmiids", "uuid", "user32", "psapi", "shlwapi", "gdi32", "vfw32", "secur32", "ws2_32", "bcrypt",
    ] {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }
}
