// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// Chat information returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInfo {
    pub id: String,
    pub display_name: String,
    pub participant_count: usize,
    pub message_count: usize,
}

/// Progress update sent to the frontend during export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportProgress {
    pub stage: String,
    pub percent: u8,
    pub message: String,
}

/// Export result returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub success: bool,
    pub job_id: Option<String>,
    pub error: Option<String>,
}

/// List available iMessage chats
#[tauri::command]
async fn list_chats() -> Result<Vec<ChatInfo>, String> {
    // TODO: Implement using imessage-exporter
    // For now, return empty list
    Ok(vec![])
}

/// Export selected chats and upload to server
#[tauri::command]
async fn export_and_upload(
    _chat_ids: Vec<String>,
    window: tauri::Window,
) -> Result<ExportResult, String> {
    // TODO: Implement export flow
    // 1. Call imessage-exporter with selected chats
    // 2. Stream progress via window.emit("export-progress", ...)
    // 3. Zip results
    // 4. Request pre-signed URL from server
    // 5. Upload to R2
    // 6. Return job ID for browser redirect

    // Emit progress example
    let _ = window.emit(
        "export-progress",
        ExportProgress {
            stage: "preparing".to_string(),
            percent: 0,
            message: "Preparing export...".to_string(),
        },
    );

    // Placeholder response
    Ok(ExportResult {
        success: false,
        job_id: None,
        error: Some("Not implemented yet".to_string()),
    })
}

/// Check if Full Disk Access is granted (macOS)
#[tauri::command]
async fn check_full_disk_access() -> Result<bool, String> {
    // TODO: Implement macOS Full Disk Access check
    // On other platforms, return true
    #[cfg(target_os = "macos")]
    {
        // Check if we can read ~/Library/Messages/chat.db
        let home = std::env::var("HOME").map_err(|e| e.to_string())?;
        let chat_db_path = format!("{}/Library/Messages/chat.db", home);
        Ok(std::path::Path::new(&chat_db_path).exists())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Open System Preferences to Full Disk Access (macOS)
#[tauri::command]
async fn open_full_disk_access_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            list_chats,
            export_and_upload,
            check_full_disk_access,
            open_full_disk_access_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
