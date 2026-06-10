//! Windows: executables whose file name contains "setup" trigger UAC
//! installer detection and demand elevation unless the embedded manifest
//! declares a requestedExecutionLevel. Cargo names this crate's test binary
//! `setup-<hash>.exe`, which would make `cargo test` fail with os error 740
//! on non-elevated shells. Embed an `asInvoker` manifest so the test
//! binaries run unelevated. No effect on other platforms.

fn main() {
    let is_windows = std::env::var_os("CARGO_CFG_WINDOWS").is_some();
    let is_msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if is_windows && is_msvc {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("setup.manifest");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        println!("cargo:rerun-if-changed=setup.manifest");
    }
}
