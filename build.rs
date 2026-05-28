//! Capture the build target triple so the self-updater can pick the matching
//! release asset at runtime (e.g. `gfit-cli-aarch64-apple-darwin`).

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo:rustc-env=GFIT_TARGET={target}");
}
