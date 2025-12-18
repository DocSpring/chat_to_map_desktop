//! Screenshot functionality for testing and documentation
//!
//! Uses xcap for cross-platform window capture.

use std::path::PathBuf;
use xcap::Window;

/// Take a screenshot of the application window and save it to the specified path.
///
/// Finds the window by matching the title prefix "ChatToMap".
pub fn capture_window(output_path: &PathBuf) -> Result<(), String> {
    let windows = Window::all().map_err(|e| format!("Failed to list windows: {e}"))?;

    // Find our window by title
    let app_window = windows
        .into_iter()
        .find(|w| {
            w.title()
                .map(|t| t.starts_with("ChatToMap"))
                .unwrap_or(false)
        })
        .ok_or_else(|| "ChatToMap window not found".to_string())?;

    // Capture the window
    let image = app_window
        .capture_image()
        .map_err(|e| format!("Failed to capture window: {e}"))?;

    // Save the image
    image
        .save(output_path)
        .map_err(|e| format!("Failed to save screenshot: {e}"))?;

    Ok(())
}

/// Screenshot configuration passed via CLI args
#[derive(Debug, Clone, Default)]
pub struct ScreenshotConfig {
    /// Run in screenshot mode
    pub enabled: bool,
    /// Theme to use: "light", "dark", or "system"
    pub theme: String,
    /// Force the FDA (Full Disk Access) check to return false
    pub force_no_fda: bool,
    /// Output directory for screenshots
    pub output_dir: PathBuf,
}

impl ScreenshotConfig {
    pub fn new() -> Self {
        Self {
            enabled: false,
            theme: "system".to_string(),
            force_no_fda: false,
            output_dir: PathBuf::from("./screenshots"),
        }
    }
}
