fn main() {
    // Only run tauri_build when building the desktop app
    if std::env::var("CARGO_FEATURE_DESKTOP").is_ok() {
        tauri_build::build();
    }
}
