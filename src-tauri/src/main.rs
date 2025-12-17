// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chat_to_map_desktop::{
    export::{export_chats, ExportProgress},
    list_chats as lib_list_chats,
    upload::{create_job, get_presigned_url, get_results_url, upload_file},
    ChatInfo,
};
use imessage_database::{tables::table::get_connection, util::dirs::default_db_path};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// Export result returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub success: bool,
    pub job_id: Option<String>,
    pub results_url: Option<String>,
    pub error: Option<String>,
}

/// List available iMessage chats
#[tauri::command]
async fn list_chats() -> Result<Vec<ChatInfo>, String> {
    lib_list_chats()
}

/// Export selected chats and upload to server
#[tauri::command]
async fn export_and_upload(
    chat_ids: Vec<i32>,
    window: tauri::Window,
) -> Result<ExportResult, String> {
    // Helper to emit progress
    let emit = |stage: &str, percent: u8, message: &str| {
        let _ = window.emit(
            "export-progress",
            ExportProgress {
                stage: stage.to_string(),
                percent,
                message: message.to_string(),
            },
        );
    };

    // Stage 1: Export messages (0-50%)
    emit("Exporting", 0, "Starting export...");

    let window_clone = window.clone();
    let progress_callback = Box::new(move |progress: ExportProgress| {
        // Scale export progress to 0-50%
        let scaled_percent = progress.percent / 2;
        let _ = window_clone.emit(
            "export-progress",
            ExportProgress {
                stage: progress.stage,
                percent: scaled_percent,
                message: progress.message,
            },
        );
    });

    let export_result =
        tokio::task::spawn_blocking(move || export_chats(&chat_ids, Some(progress_callback)))
            .await
            .map_err(|e| format!("Export task failed: {e}"))?
            .map_err(|e| format!("Export failed: {e}"))?;

    // Stage 2: Get pre-signed URL (50-55%)
    emit("Uploading", 50, "Preparing upload...");

    let presign_response = get_presigned_url()
        .await
        .map_err(|e| format!("Failed to get upload URL: {e}"))?;

    // Stage 3: Upload file (55-90%)
    emit("Uploading", 55, "Uploading to server...");

    let window_clone = window.clone();
    let upload_callback = Box::new(move |percent: u8, message: String| {
        // Scale upload progress to 55-90%
        let scaled_percent = 55 + (percent * 35 / 100);
        let _ = window_clone.emit(
            "export-progress",
            ExportProgress {
                stage: "Uploading".to_string(),
                percent: scaled_percent,
                message,
            },
        );
    });

    upload_file(
        &export_result.zip_path,
        &presign_response.upload_url,
        Some(upload_callback),
    )
    .await
    .map_err(|e| format!("Upload failed: {e}"))?;

    // Stage 4: Create job (90-95%)
    emit("Processing", 90, "Creating processing job...");

    let job_response = create_job(
        &presign_response.file_key,
        export_result.chat_count,
        export_result.total_messages,
    )
    .await
    .map_err(|e| format!("Failed to create job: {e}"))?;

    // Stage 5: Complete (95-100%)
    let results_url = get_results_url(&job_response.job_id);
    emit("Complete", 100, "Export complete!");

    // Open browser to results page
    if let Err(e) = open::that(&results_url) {
        eprintln!("Failed to open browser: {e}");
    }

    Ok(ExportResult {
        success: true,
        job_id: Some(job_response.job_id),
        results_url: Some(results_url),
        error: None,
    })
}

/// Check if Full Disk Access is granted (macOS)
#[tauri::command]
async fn check_full_disk_access() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        // Check if we can actually read the database
        let db_path = default_db_path();
        if !db_path.exists() {
            return Ok(false);
        }

        // Try to open the database - this will fail without FDA
        match get_connection(&db_path) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
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

/// Save export locally (fallback when upload fails)
#[tauri::command]
async fn save_export_locally(chat_ids: Vec<i32>, save_path: String) -> Result<String, String> {
    let export_result = tokio::task::spawn_blocking(move || export_chats(&chat_ids, None))
        .await
        .map_err(|e| format!("Export task failed: {e}"))?
        .map_err(|e| format!("Export failed: {e}"))?;

    // Copy zip to user-specified location
    std::fs::copy(&export_result.zip_path, &save_path)
        .map_err(|e| format!("Failed to save file: {e}"))?;

    Ok(save_path)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            list_chats,
            export_and_upload,
            check_full_disk_access,
            open_full_disk_access_settings,
            save_export_locally,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
