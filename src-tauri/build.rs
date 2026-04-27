use std::env;

fn main() {
    // Run tauri-build only when the desktop binary is being built.
    if env::var("CARGO_FEATURE_DESKTOP").is_ok() {
        tauri_build::build();
    }
}
