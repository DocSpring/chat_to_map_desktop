// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;

use chat_to_map_desktop::{
    export::{export_chats, ExportProgress},
    list_chats as lib_list_chats,
    screenshot::{capture_window, ScreenshotConfig},
    upload::{complete_upload, get_presigned_url, get_results_url, upload_file},
    validate_chat_db as lib_validate_chat_db, ChatInfo,
};
use clap::Parser;
use imessage_database::{tables::table::get_connection, util::dirs::default_db_path};
use serde::{Deserialize, Serialize};
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::Emitter;

/// CLI arguments for the desktop app
#[derive(Parser, Debug)]
#[command(name = "chat-to-map-desktop")]
#[command(about = "ChatToMap Desktop - Export iMessage chats")]
struct Args {
    /// Run in screenshot mode for testing/documentation
    #[arg(long)]
    screenshot_mode: bool,

    /// Theme to use: light, dark, or system (default: system)
    #[arg(long, default_value = "system")]
    theme: String,

    /// Force FDA (Full Disk Access) check to return false
    #[arg(long)]
    force_no_fda: bool,

    /// Output directory for screenshots (default: ./screenshots)
    #[arg(long, default_value = "./screenshots")]
    output_dir: PathBuf,
}

/// App state for screenshot configuration
struct AppState {
    screenshot_config: Mutex<ScreenshotConfig>,
}

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
fn list_chats(custom_db_path: Option<String>) -> Result<Vec<ChatInfo>, String> {
    eprintln!(
        "[tauri::list_chats] Command invoked, custom_db_path: {:?}",
        custom_db_path
    );
    let path = custom_db_path.as_ref().map(PathBuf::from);
    let result = lib_list_chats(path.as_deref());
    eprintln!(
        "[tauri::list_chats] Result: {:?}",
        result.as_ref().map(|v| v.len())
    );
    result
}

/// Validate that a file is a valid iMessage chat.db database
#[tauri::command]
fn validate_chat_db(path: String) -> bool {
    eprintln!("[tauri::validate_chat_db] Validating: {}", path);
    lib_validate_chat_db(&PathBuf::from(path))
}

/// Export selected chats and upload to server
#[tauri::command]
async fn export_and_upload(
    chat_ids: Vec<i32>,
    custom_db_path: Option<String>,
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

    let db_path = custom_db_path.map(PathBuf::from);
    let export_result = tokio::task::spawn_blocking(move || {
        export_chats(&chat_ids, Some(progress_callback), db_path.as_deref())
    })
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

    // Stage 4: Complete upload and start processing (90-95%)
    emit("Processing", 90, "Starting processing...");

    let job_response = complete_upload(&presign_response.job_id)
        .await
        .map_err(|e| format!("Failed to start processing: {e}"))?;

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
/// Respects the --force-no-fda flag for screenshot testing
#[tauri::command]
fn check_full_disk_access(state: tauri::State<AppState>) -> Result<bool, String> {
    eprintln!("[check_full_disk_access] Checking...");

    // Check if we're forcing FDA to be denied (for screenshot mode)
    let config = state.screenshot_config.lock().unwrap();
    if config.force_no_fda {
        eprintln!("[check_full_disk_access] Force no FDA enabled");
        return Ok(false);
    }
    drop(config);

    #[cfg(target_os = "macos")]
    {
        // Check if we can actually read the database
        let db_path = default_db_path();
        eprintln!("[check_full_disk_access] DB path: {:?}", db_path);
        if !db_path.exists() {
            eprintln!("[check_full_disk_access] DB does not exist");
            return Ok(false);
        }

        // Try to open the database - this will fail without FDA
        match get_connection(&db_path) {
            Ok(_) => {
                eprintln!("[check_full_disk_access] FDA granted (can open DB)");
                Ok(true)
            }
            Err(e) => {
                eprintln!("[check_full_disk_access] FDA denied: {:?}", e);
                Ok(false)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Open System Preferences to Full Disk Access (macOS)
#[tauri::command]
fn open_full_disk_access_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Check if Contacts access is granted (macOS)
#[tauri::command]
fn check_contacts_access() -> Result<bool, String> {
    eprintln!("[check_contacts_access] Checking...");

    #[cfg(target_os = "macos")]
    {
        use chat_to_map_desktop::contacts::ContactsIndex;

        // Try to build the contacts index - this will fail without Contacts permission
        match ContactsIndex::build(None) {
            Ok(index) => {
                let has_contacts = !index.is_empty();
                eprintln!(
                    "[check_contacts_access] Contacts access granted, {} entries",
                    index.len()
                );
                // If the index is empty, it might mean no permission OR no contacts
                // We return true if we could read the database (even if empty)
                Ok(has_contacts || index.is_empty())
            }
            Err(e) => {
                eprintln!("[check_contacts_access] Contacts access denied: {:?}", e);
                Ok(false)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        // On non-macOS platforms, contacts aren't available
        Ok(false)
    }
}

/// Open System Preferences to Contacts (macOS)
#[tauri::command]
fn open_contacts_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Contacts")
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Screenshot mode config returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotConfigResponse {
    pub enabled: bool,
    pub theme: String,
    pub force_no_fda: bool,
    pub output_dir: String,
}

/// Get screenshot configuration (for frontend to detect screenshot mode)
#[tauri::command]
fn get_screenshot_config(state: tauri::State<AppState>) -> ScreenshotConfigResponse {
    let config = state.screenshot_config.lock().unwrap();
    ScreenshotConfigResponse {
        enabled: config.enabled,
        theme: config.theme.clone(),
        force_no_fda: config.force_no_fda,
        output_dir: config.output_dir.to_string_lossy().to_string(),
    }
}

/// Open the Open Source Licenses (CREDITS.md)
#[tauri::command]
fn open_licenses() -> Result<(), String> {
    // Get path to CREDITS.md relative to the executable
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let app_dir = exe_path.parent().ok_or("No parent directory")?;

    // In development, CREDITS.md is at the repo root
    // In production bundle, it's in Resources
    let credits_paths = vec![
        app_dir.join("../Resources/CREDITS.md"), // macOS bundle
        app_dir.join("../../CREDITS.md"),        // Development (src-tauri/target/debug)
        app_dir.join("../../../CREDITS.md"),     // Development (nested)
        std::path::PathBuf::from("CREDITS.md"),  // Current dir fallback
    ];

    for path in credits_paths {
        if path.exists() {
            return open::that(&path).map_err(|e| format!("Failed to open CREDITS.md: {e}"));
        }
    }

    // Fallback: open GitHub repo
    open::that("https://github.com/DocSpring/chat_to_map_desktop/blob/main/CREDITS.md")
        .map_err(|e| format!("Failed to open URL: {e}"))
}

/// Take a screenshot and save it to the specified filename
#[tauri::command]
fn take_screenshot(state: tauri::State<AppState>, filename: String) -> Result<String, String> {
    let config = state.screenshot_config.lock().unwrap();
    let output_path = config.output_dir.join(&filename);
    drop(config);

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create output directory: {e}"))?;
    }

    capture_window(&output_path)?;
    Ok(output_path.to_string_lossy().to_string())
}

fn main() {
    // Parse CLI arguments
    let args = Args::parse();

    // Build screenshot config from args
    let screenshot_config = ScreenshotConfig {
        enabled: args.screenshot_mode,
        theme: args.theme,
        force_no_fda: args.force_no_fda,
        output_dir: args.output_dir,
    };

    eprintln!("[main] Screenshot mode: {}", screenshot_config.enabled);
    eprintln!("[main] Theme: {}", screenshot_config.theme);
    eprintln!("[main] Force no FDA: {}", screenshot_config.force_no_fda);

    let app_state = AppState {
        screenshot_config: Mutex::new(screenshot_config),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(|app| {
            // Build Help menu with Open Source Licenses item
            let licenses_item = MenuItemBuilder::new("Open Source Licenses")
                .id("open_licenses")
                .build(app)?;

            let help_menu = SubmenuBuilder::new(app, "Help")
                .item(&licenses_item)
                .build()?;

            let menu = MenuBuilder::new(app).item(&help_menu).build()?;

            app.set_menu(menu)?;

            Ok(())
        })
        .on_menu_event(|_app, event| {
            if event.id().as_ref() == "open_licenses" {
                if let Err(e) = open_licenses() {
                    eprintln!("Failed to open licenses: {e}");
                }
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_chats,
            validate_chat_db,
            export_and_upload,
            check_full_disk_access,
            open_full_disk_access_settings,
            check_contacts_access,
            open_contacts_settings,
            get_screenshot_config,
            take_screenshot,
            open_licenses,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
