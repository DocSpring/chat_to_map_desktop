/*!
 * ChatToMap Desktop - Shared library code
 *
 * This module contains the core functionality shared between
 * the desktop app (Tauri) and the CLI debugging tool.
 */

pub mod contacts;

#[cfg(test)]
pub mod test_fixtures;

use std::collections::HashMap;

use contacts::{ContactsIndex, Name};
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

/// Chat information returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInfo {
    pub id: i32,
    /// Resolved contact name or fallback to identifier
    pub display_name: String,
    /// Raw identifier (phone number, email, or group chat ID)
    pub chat_identifier: String,
    pub service: String,
    pub participant_count: usize,
    pub message_count: usize,
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
pub fn resolve_chat_display_name(
    chat: &Chat,
    chat_participants: Option<&std::collections::BTreeSet<i32>>,
    participants_map: &HashMap<i32, Name>,
    deduped_handles: &HashMap<i32, i32>,
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
                // FIX: Translate handle_id to deduped_id before lookup
                if let Some(&deduped_id) = deduped_handles.get(&handle_id) {
                    if let Some(name) = participants_map.get(&deduped_id) {
                        let display = name.get_display_name();
                        if !display.is_empty() {
                            return display.to_string();
                        }
                    }
                }
            }
        }
    }

    // Fallback to chat_identifier
    chat.chat_identifier.clone()
}

/// List available iMessage chats
pub fn list_chats() -> Result<Vec<ChatInfo>, String> {
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

            let display_name =
                resolve_chat_display_name(&chat, participants, &participants_map, &deduped_handles);

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
