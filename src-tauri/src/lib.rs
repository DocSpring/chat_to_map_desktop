/*!
 * ChatToMap Desktop - Shared library code
 *
 * This module contains the core functionality shared between
 * the desktop app (Tauri) and the CLI debugging tool.
 */

pub mod api;
pub mod contacts;
pub mod export;
pub mod screenshot;
pub mod upload;

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

/// Chat statistics (message count and last message timestamp)
struct ChatStats {
    message_count: usize,
    last_message_date: i64,
}

/// Get message counts and last message date per chat using custom SQL
fn get_chat_stats(
    db: &rusqlite::Connection,
) -> Result<HashMap<i32, ChatStats>, imessage_database::error::table::TableError> {
    let mut stats = HashMap::new();

    let mut stmt = db.prepare(
        "SELECT cmj.chat_id, COUNT(*) as count, MAX(m.date) as last_date
         FROM chat_message_join cmj
         JOIN message m ON cmj.message_id = m.ROWID
         GROUP BY cmj.chat_id",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i32>(0)?,
            row.get::<_, usize>(1)?,
            row.get::<_, i64>(2).unwrap_or(0),
        ))
    })?;

    for (chat_id, count, last_date) in rows.flatten() {
        stats.insert(
            chat_id,
            ChatStats {
                message_count: count,
                last_message_date: last_date,
            },
        );
    }

    Ok(stats)
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
    eprintln!("[list_chats] Starting...");

    // Get database path
    let db_path = default_db_path();
    eprintln!("[list_chats] DB path: {:?}", db_path);

    // Connect to database
    let db = get_connection(&db_path).map_err(|e| format!("Failed to connect to database: {e}"))?;
    eprintln!("[list_chats] Connected to database");

    // Build contacts index for name resolution
    eprintln!("[list_chats] Building contacts index...");
    let contacts_index = ContactsIndex::build(None).unwrap_or_default();
    eprintln!("[list_chats] Contacts index built");

    // Cache all chats
    eprintln!("[list_chats] Loading chats...");
    let chats = Chat::cache(&db).map_err(|e| format!("Failed to load chats: {e}"))?;
    eprintln!("[list_chats] Loaded {} chats", chats.len());

    // Cache handles (contacts)
    eprintln!("[list_chats] Loading handles...");
    let handles = Handle::cache(&db).map_err(|e| format!("Failed to load handles: {e}"))?;
    let deduped_handles = Handle::dedupe(&handles);
    eprintln!("[list_chats] Loaded {} handles", handles.len());

    // Build participants map with resolved names
    let participants_map = contacts_index.build_participants_map(&handles, &deduped_handles);

    // Cache chat participants (chat_id -> set of handle_ids)
    eprintln!("[list_chats] Loading chat participants...");
    let chat_participants =
        ChatToHandle::cache(&db).map_err(|e| format!("Failed to load participants: {e}"))?;
    eprintln!(
        "[list_chats] Loaded participants for {} chats",
        chat_participants.len()
    );

    // Get chat stats (message counts and last message dates)
    eprintln!("[list_chats] Getting chat stats...");
    let chat_stats = get_chat_stats(&db).map_err(|e| format!("Failed to get chat stats: {e}"))?;
    eprintln!("[list_chats] Got chat stats");

    // Build result with last_message_date for sorting
    let mut result: Vec<(ChatInfo, i64)> = chats
        .into_iter()
        .map(|(id, chat)| {
            let participants = chat_participants.get(&id);
            let participant_count = participants.map(|p| p.len()).unwrap_or(0);
            let stats = chat_stats.get(&id);
            let message_count = stats.map(|s| s.message_count).unwrap_or(0);
            let last_message_date = stats.map(|s| s.last_message_date).unwrap_or(0);

            let display_name =
                resolve_chat_display_name(&chat, participants, &participants_map, &deduped_handles);

            (
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
                },
                last_message_date,
            )
        })
        .collect();

    // Sort by last message date descending (most recent first)
    result.sort_by(|a, b| b.1.cmp(&a.1));

    // Extract just the ChatInfo
    let result: Vec<ChatInfo> = result.into_iter().map(|(info, _)| info).collect();

    eprintln!("[list_chats] Done! Returning {} chats", result.len());
    Ok(result)
}
