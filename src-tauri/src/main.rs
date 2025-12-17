// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod contacts;

use std::collections::HashMap;

use contacts::ContactsIndex;
use imessage_database::{
    tables::{
        chat::Chat,
        chat_handle::ChatToHandle,
        handle::Handle,
        table::{get_connection, Cacheable, Deduplicate},
    },
    util::dirs::default_db_path,
};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// Chat information returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInfo {
    pub id: i32,
    /// Resolved contact name (e.g., "Masha Broadbent") or fallback to identifier
    pub display_name: String,
    /// Raw identifier (phone number, email, or group chat ID)
    pub chat_identifier: String,
    pub service: String,
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

/// Get message counts per chat using custom SQL
fn get_message_counts(
    db: &rusqlite::Connection,
) -> Result<HashMap<i32, usize>, imessage_database::error::table::TableError> {
    let mut counts = HashMap::new();

    let mut stmt =
        db.prepare("SELECT chat_id, COUNT(*) as count FROM chat_message_join GROUP BY chat_id")?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, usize>(1)?))
    })?;

    for (chat_id, count) in rows.flatten() {
        counts.insert(chat_id, count);
    }

    Ok(counts)
}

/// Resolve a display name for a chat, using contacts if available
fn resolve_chat_display_name(
    chat: &Chat,
    chat_participants: Option<&std::collections::BTreeSet<i32>>,
    participants_map: &HashMap<i32, contacts::Name>,
) -> String {
    // If chat has a custom display_name, use it
    if let Some(name) = chat.display_name.as_ref() {
        if !name.is_empty() {
            return name.clone();
        }
    }

    // For 1:1 chats, try to resolve the participant's name
    if let Some(participant_ids) = chat_participants {
        if participant_ids.len() == 1 {
            if let Some(&handle_id) = participant_ids.iter().next() {
                if let Some(name) = participants_map.get(&handle_id) {
                    let display = name.get_display_name();
                    if !display.is_empty() {
                        return display.to_string();
                    }
                }
            }
        }
    }

    // Fallback to chat_identifier
    chat.chat_identifier.clone()
}

/// List available iMessage chats
#[tauri::command]
async fn list_chats() -> Result<Vec<ChatInfo>, String> {
    // Get database path
    let db_path = default_db_path();

    // Connect to database
    let db = get_connection(&db_path).map_err(|e| format!("Failed to connect to database: {e}"))?;

    // Build contacts index for name resolution
    let contacts_index = ContactsIndex::build(None).unwrap_or_default();

    // Cache all chats
    let chats = Chat::cache(&db).map_err(|e| format!("Failed to load chats: {e}"))?;

    // Cache handles (contacts)
    let handles = Handle::cache(&db).map_err(|e| format!("Failed to load handles: {e}"))?;
    let deduped_handles = Handle::dedupe(&handles);

    // Build participants map with resolved names
    let participants_map = contacts_index.build_participants_map(&handles, &deduped_handles);

    // Cache chat participants (chat_id -> set of handle_ids)
    let chat_participants =
        ChatToHandle::cache(&db).map_err(|e| format!("Failed to load participants: {e}"))?;

    // Get message counts
    let message_counts =
        get_message_counts(&db).map_err(|e| format!("Failed to get message counts: {e}"))?;

    // Build result
    let mut result: Vec<ChatInfo> = chats
        .into_iter()
        .map(|(id, chat)| {
            let participants = chat_participants.get(&id);
            let participant_count = participants.map(|p| p.len()).unwrap_or(0);
            let message_count = message_counts.get(&id).copied().unwrap_or(0);

            let display_name = resolve_chat_display_name(&chat, participants, &participants_map);

            ChatInfo {
                id,
                display_name,
                chat_identifier: chat.chat_identifier.clone(),
                service: chat
                    .service_name
                    .as_deref()
                    .unwrap_or("Unknown")
                    .to_string(),
                participant_count,
                message_count,
            }
        })
        .collect();

    // Sort by message count descending (most active chats first)
    result.sort_by(|a, b| b.message_count.cmp(&a.message_count));

    Ok(result)
}

/// Export selected chats and upload to server
#[tauri::command]
async fn export_and_upload(
    _chat_ids: Vec<i32>,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that we can read the iMessage database and resolve contacts
    /// Run with: cargo test --package chat-to-map-desktop -- --nocapture
    #[test]
    fn test_list_chats_with_contact_resolution() {
        let db_path = default_db_path();

        // Check if database exists
        if !db_path.exists() {
            println!(
                "Skipping test: iMessage database not found at {:?}",
                db_path
            );
            println!("This test requires macOS with an iMessage database.");
            return;
        }

        // Try to connect
        let db = match get_connection(&db_path) {
            Ok(db) => db,
            Err(e) => {
                println!("Skipping test: Could not connect to database: {}", e);
                println!("This may require Full Disk Access permission.");
                return;
            }
        };

        // Build contacts index
        let contacts_index = ContactsIndex::build(None).unwrap_or_default();
        println!("Contacts index: {} entries", contacts_index.len());

        // Cache chats
        let chats = Chat::cache(&db).expect("Failed to load chats");
        println!("Found {} chats", chats.len());

        // Cache handles
        let handles = Handle::cache(&db).expect("Failed to load handles");
        let deduped_handles = Handle::dedupe(&handles);
        println!("Found {} handles", handles.len());

        // Build participants map
        let participants_map = contacts_index.build_participants_map(&handles, &deduped_handles);
        println!(
            "Resolved {} participants with names",
            participants_map
                .values()
                .filter(|n| !n.full.is_empty())
                .count()
        );

        // Cache chat participants
        let chat_participants = ChatToHandle::cache(&db).expect("Failed to load participants");

        // Get message counts
        let message_counts = get_message_counts(&db).expect("Failed to get message counts");

        // Build and print first 10 chats
        let mut result: Vec<ChatInfo> = chats
            .into_iter()
            .map(|(id, chat)| {
                let participants = chat_participants.get(&id);
                let participant_count = participants.map(|p| p.len()).unwrap_or(0);
                let message_count = message_counts.get(&id).copied().unwrap_or(0);
                let display_name =
                    resolve_chat_display_name(&chat, participants, &participants_map);

                ChatInfo {
                    id,
                    display_name,
                    chat_identifier: chat.chat_identifier.clone(),
                    service: chat
                        .service_name
                        .as_deref()
                        .unwrap_or("Unknown")
                        .to_string(),
                    participant_count,
                    message_count,
                }
            })
            .collect();

        result.sort_by(|a, b| b.message_count.cmp(&a.message_count));

        println!("\nTop 10 chats by message count:");
        for (i, chat) in result.iter().take(10).enumerate() {
            let resolved = if chat.display_name != chat.chat_identifier {
                " ✓"
            } else {
                ""
            };
            println!(
                "  {}. {}{} ({}) - {} messages",
                i + 1,
                chat.display_name,
                resolved,
                chat.service,
                chat.message_count
            );
        }

        assert!(!result.is_empty(), "Should have at least one chat");
    }
}
