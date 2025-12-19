/*!
 * Message export functionality
 *
 * Exports iMessage messages for selected chats to a zip file
 * compatible with the ChatToMap SaaS processing pipeline.
 */

use std::{
    collections::{BTreeSet, HashMap},
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

use chrono::{DateTime, Local, TimeZone};
use imessage_database::{
    tables::{
        chat::Chat,
        handle::Handle,
        messages::Message,
        table::{get_connection, Cacheable, Deduplicate, Table},
    },
    util::{dirs::default_db_path, query_context::QueryContext},
};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use zip::{write::SimpleFileOptions, ZipWriter};

use crate::contacts::{ContactsIndex, Name};

// =============================================================================
// Types
// =============================================================================

/// A single exported message in our JSON format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedMessage {
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Sender name or phone/email
    pub sender: String,
    /// Whether this message is from the device owner
    pub is_from_me: bool,
    /// Message text content
    pub text: String,
}

/// Metadata about an exported chat
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportedChatMeta {
    /// Chat display name
    pub name: String,
    /// Raw chat identifier (phone number, email, or group ID)
    pub identifier: String,
    /// Service (iMessage, SMS)
    pub service: String,
    /// Number of messages exported
    pub message_count: usize,
}

/// Complete export data for a single chat
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportedChat {
    pub meta: ExportedChatMeta,
    pub messages: Vec<ExportedMessage>,
}

/// Progress callback signature
pub type ProgressCallback = Box<dyn Fn(ExportProgress) + Send + Sync>;

/// Export progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportProgress {
    pub stage: String,
    pub percent: u8,
    pub message: String,
}

/// Export result
#[derive(Debug)]
pub struct ExportResult {
    /// Path to the zip file
    pub zip_path: PathBuf,
    /// Temporary directory (kept alive until result is dropped)
    pub _temp_dir: TempDir,
    /// Total messages exported
    pub total_messages: usize,
    /// Number of chats exported
    pub chat_count: usize,
}

// =============================================================================
// Constants
// =============================================================================

/// iMessage timestamp epoch offset (2001-01-01 vs 1970-01-01)
const APPLE_EPOCH_OFFSET: i64 = 978_307_200;

/// Nanoseconds factor for iMessage timestamps
const TIMESTAMP_FACTOR: i64 = 1_000_000_000;

// =============================================================================
// Export Implementation
// =============================================================================

/// Export messages for selected chats to a zip file
///
/// # Arguments
/// * `chat_ids` - List of chat ROWIDs to export
/// * `progress_callback` - Optional callback for progress updates
///
/// # Returns
/// * `ExportResult` containing the zip file path and metadata
pub fn export_chats(
    chat_ids: &[i32],
    progress_callback: Option<ProgressCallback>,
    custom_db_path: Option<&std::path::Path>,
) -> Result<ExportResult, String> {
    let emit_progress = |progress: ExportProgress| {
        if let Some(ref cb) = progress_callback {
            cb(progress);
        }
    };

    emit_progress(ExportProgress {
        stage: "Initializing".to_string(),
        percent: 0,
        message: "Connecting to iMessage database...".to_string(),
    });

    // Connect to database
    let db_path = custom_db_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(default_db_path);
    let db = get_connection(&db_path).map_err(|e| format!("Failed to connect to database: {e}"))?;

    // Build contacts index for name resolution
    let contacts_index = ContactsIndex::build(None).unwrap_or_default();

    // Cache handles for participant name lookup
    let handles = Handle::cache(&db).map_err(|e| format!("Failed to load handles: {e}"))?;
    let deduped_handles = Handle::dedupe(&handles);
    let participants_map = contacts_index.build_participants_map(&handles, &deduped_handles);

    // Cache chats for metadata
    let chats = Chat::cache(&db).map_err(|e| format!("Failed to load chats: {e}"))?;

    emit_progress(ExportProgress {
        stage: "Preparing".to_string(),
        percent: 5,
        message: "Counting messages...".to_string(),
    });

    // Set up query context with selected chat IDs
    let mut query_context = QueryContext::default();
    query_context.set_selected_chat_ids(chat_ids.iter().copied().collect::<BTreeSet<_>>());

    // Get total message count for progress tracking
    let total_messages = Message::get_count(&db, &query_context)
        .map_err(|e| format!("Failed to count messages: {e}"))?;

    emit_progress(ExportProgress {
        stage: "Exporting".to_string(),
        percent: 10,
        message: format!("Exporting {} messages...", total_messages),
    });

    // Stream messages and group by chat
    let mut messages_by_chat: HashMap<i32, Vec<ExportedMessage>> = HashMap::new();
    let mut processed: usize = 0;

    Message::stream(&db, |message_result| {
        match message_result {
            Ok(mut message) => {
                // Filter to selected chats
                if let Some(chat_id) = message.chat_id {
                    if chat_ids.contains(&chat_id) {
                        // Generate text content (deserializes protobuf/plist)
                        let _ = message.generate_text(&db);

                        // Get sender name
                        let sender = get_sender_name(
                            &message,
                            &handles,
                            &deduped_handles,
                            &participants_map,
                        );

                        // Convert timestamp
                        let timestamp = format_timestamp(message.date);

                        // Get message text (skip empty messages)
                        if let Some(text) = message.text.as_ref() {
                            if !text.is_empty() {
                                let exported = ExportedMessage {
                                    timestamp,
                                    sender,
                                    is_from_me: message.is_from_me,
                                    text: text.clone(),
                                };

                                messages_by_chat.entry(chat_id).or_default().push(exported);
                            }
                        }

                        processed += 1;

                        // Update progress every 100 messages
                        if processed % 100 == 0 {
                            let percent =
                                10 + (processed as u64 * 70 / total_messages.max(1)) as u8;
                            emit_progress(ExportProgress {
                                stage: "Exporting".to_string(),
                                percent: percent.min(80),
                                message: format!(
                                    "Processed {} of {} messages",
                                    processed, total_messages
                                ),
                            });
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading message: {:?}", e);
            }
        }
        Ok::<(), String>(())
    })
    .map_err(|e| format!("Failed to stream messages: {e}"))?;

    emit_progress(ExportProgress {
        stage: "Packaging".to_string(),
        percent: 85,
        message: "Creating export package...".to_string(),
    });

    // Create temp directory for export
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp directory: {e}"))?;

    // Build exported chats
    let mut exported_chats = Vec::new();
    for (&chat_id, messages) in &messages_by_chat {
        let chat = chats.get(&chat_id);
        let meta = ExportedChatMeta {
            name: chat
                .and_then(|c| c.display_name.clone())
                .unwrap_or_else(|| format!("Chat {}", chat_id)),
            identifier: chat.map(|c| c.chat_identifier.clone()).unwrap_or_default(),
            service: chat
                .and_then(|c| c.service_name.clone())
                .unwrap_or_else(|| "Unknown".to_string()),
            message_count: messages.len(),
        };

        exported_chats.push(ExportedChat {
            meta,
            messages: messages.clone(),
        });
    }

    // Sort by message count descending
    exported_chats.sort_by(|a, b| b.messages.len().cmp(&a.messages.len()));

    // Write each chat to a separate JSON file and create zip
    let zip_path = temp_dir.path().join("export.zip");
    let zip_file = File::create(&zip_path).map_err(|e| format!("Failed to create zip: {e}"))?;
    let mut zip = ZipWriter::new(BufWriter::new(zip_file));

    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Write manifest
    let manifest = serde_json::json!({
        "version": "1.0",
        "source": "imessage",
        "export_date": chrono::Utc::now().to_rfc3339(),
        "chat_count": exported_chats.len(),
        "total_messages": processed,
    });

    zip.start_file("manifest.json", options)
        .map_err(|e| format!("Failed to write manifest: {e}"))?;
    zip.write_all(serde_json::to_string_pretty(&manifest).unwrap().as_bytes())
        .map_err(|e| format!("Failed to write manifest: {e}"))?;

    // Write each chat
    for (i, chat) in exported_chats.iter().enumerate() {
        let filename = format!("chat_{:03}.json", i);
        zip.start_file(&filename, options)
            .map_err(|e| format!("Failed to write chat: {e}"))?;
        zip.write_all(serde_json::to_string_pretty(&chat).unwrap().as_bytes())
            .map_err(|e| format!("Failed to write chat: {e}"))?;
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize zip: {e}"))?;

    emit_progress(ExportProgress {
        stage: "Complete".to_string(),
        percent: 100,
        message: format!(
            "Exported {} messages from {} chats",
            processed,
            exported_chats.len()
        ),
    });

    Ok(ExportResult {
        zip_path,
        _temp_dir: temp_dir,
        total_messages: processed,
        chat_count: exported_chats.len(),
    })
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Get sender name for a message
fn get_sender_name(
    message: &Message,
    handles: &HashMap<i32, String>,
    deduped_handles: &HashMap<i32, i32>,
    participants_map: &HashMap<i32, Name>,
) -> String {
    if message.is_from_me {
        return "Me".to_string();
    }

    if let Some(handle_id) = message.handle_id {
        // Look up deduped ID first
        if let Some(&deduped_id) = deduped_handles.get(&handle_id) {
            if let Some(name) = participants_map.get(&deduped_id) {
                let display = name.get_display_name();
                if !display.is_empty() {
                    return display.to_string();
                }
            }
        }

        // Fall back to raw handle ID (phone/email)
        if let Some(handle_id_str) = handles.get(&handle_id) {
            return handle_id_str.clone();
        }
    }

    "Unknown".to_string()
}

/// Convert iMessage timestamp to ISO 8601 string
fn format_timestamp(imessage_timestamp: i64) -> String {
    // iMessage timestamps are nanoseconds since 2001-01-01
    let unix_timestamp = (imessage_timestamp / TIMESTAMP_FACTOR) + APPLE_EPOCH_OFFSET;

    match DateTime::from_timestamp(unix_timestamp, 0) {
        Some(dt) => {
            let local: DateTime<Local> = Local.from_utc_datetime(&dt.naive_utc());
            local.to_rfc3339()
        }
        None => chrono::Utc::now().to_rfc3339(),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp() {
        // 2024-01-01 00:00:00 UTC in iMessage timestamp format
        // Unix: 1704067200, iMessage: (1704067200 - 978307200) * 1_000_000_000
        let imessage_ts = (1704067200_i64 - APPLE_EPOCH_OFFSET) * TIMESTAMP_FACTOR;
        let result = format_timestamp(imessage_ts);

        // Should contain 2024-01-01
        assert!(result.contains("2024-01-01") || result.contains("2023-12-31"));
    }

    #[test]
    fn test_exported_message_serialization() {
        let msg = ExportedMessage {
            timestamp: "2024-01-01T12:00:00+00:00".to_string(),
            sender: "Alice".to_string(),
            is_from_me: false,
            text: "Hello world".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("Alice"));
        assert!(json.contains("Hello world"));
    }
}
