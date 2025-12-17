/*!
 * Test fixtures module - FactoryBot-style test data builders
 *
 * Provides builders for creating real SQLite test databases using
 * the actual iMessage and AddressBook schemas.
 *
 * Usage:
 * ```rust
 * let db = TestIMessageDb::new()
 *     .with_handle(HandleBuilder::new("+15551234567").service("iMessage"))
 *     .with_chat(ChatBuilder::new("iMessage;-;+15551234567"))
 *     .build();
 * ```
 */

mod addressbook;
mod imessage;

pub use addressbook::{ContactBuilder, TestAddressBookDb};
pub use imessage::{ChatBuilder, HandleBuilder, MessageBuilder, TestIMessageDb};

use rusqlite::Result;

/// Create a standard test scenario with known contacts and chats
#[allow(dead_code)]
pub fn standard_test_scenario() -> Result<(TestIMessageDb, TestAddressBookDb)> {
    let mut imessage_db = TestIMessageDb::new()?;
    let mut contacts_db = TestAddressBookDb::new()?;

    // Create contacts
    contacts_db.contact(
        ContactBuilder::new()
            .first_name("Alice")
            .last_name("Johnson")
            .phone("+15551234567"),
    )?;

    contacts_db.contact(
        ContactBuilder::new()
            .first_name("Bob")
            .last_name("Williams")
            .phone("+6421555123"),
    )?;

    contacts_db.contact(
        ContactBuilder::new()
            .first_name("Charlie")
            .last_name("Brown")
            .email("charlie@example.com"),
    )?;

    // Create handles
    let alice_handle = imessage_db.handle(HandleBuilder::new("+15551234567"))?;
    let bob_handle = imessage_db.handle(HandleBuilder::new("+6421555123"))?;
    let charlie_handle = imessage_db.handle(HandleBuilder::new("charlie@example.com"))?;
    let unknown_handle = imessage_db.handle(HandleBuilder::new("+6421999888"))?;

    // Create chats
    let alice_chat = imessage_db.chat(ChatBuilder::new("iMessage;-;+15551234567"))?;
    let bob_chat = imessage_db.chat(ChatBuilder::new("iMessage;-;+6421555123"))?;
    let charlie_chat = imessage_db.chat(ChatBuilder::new("iMessage;-;charlie@example.com"))?;
    let unknown_chat = imessage_db.chat(ChatBuilder::new("iMessage;-;+6421999888"))?;
    let group_chat = imessage_db.chat(
        ChatBuilder::new("chat123456")
            .group()
            .display_name("Family Group"),
    )?;

    // Link handles to chats
    imessage_db.chat_handle(alice_chat, alice_handle)?;
    imessage_db.chat_handle(bob_chat, bob_handle)?;
    imessage_db.chat_handle(charlie_chat, charlie_handle)?;
    imessage_db.chat_handle(unknown_chat, unknown_handle)?;
    imessage_db.chat_handle(group_chat, alice_handle)?;
    imessage_db.chat_handle(group_chat, bob_handle)?;

    // Add some messages
    imessage_db.message(
        MessageBuilder::new()
            .text("Hello Alice!")
            .handle(alice_handle)
            .chat(alice_chat)
            .date(1000000),
    )?;
    imessage_db.message(
        MessageBuilder::new()
            .text("Hi Bob!")
            .handle(bob_handle)
            .chat(bob_chat)
            .date(2000000),
    )?;

    Ok((imessage_db, contacts_db))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_imessage_db() {
        let db = TestIMessageDb::new().unwrap();
        let tables: Vec<String> = db
            .conn()
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"handle".to_string()));
        assert!(tables.contains(&"chat".to_string()));
        assert!(tables.contains(&"message".to_string()));
    }

    #[test]
    fn test_handle_builder() {
        let mut db = TestIMessageDb::new().unwrap();
        let handle_id = db
            .handle(
                HandleBuilder::new("+15551234567")
                    .country("US")
                    .service("iMessage"),
            )
            .unwrap();

        let (id, service): (String, String) = db
            .conn()
            .query_row(
                "SELECT id, service FROM handle WHERE ROWID = ?",
                [handle_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(id, "+15551234567");
        assert_eq!(service, "iMessage");
    }

    #[test]
    fn test_chat_builder() {
        let mut db = TestIMessageDb::new().unwrap();
        let chat_id = db
            .chat(ChatBuilder::new("iMessage;-;+15551234567").display_name("Alice"))
            .unwrap();

        let (identifier, name): (String, Option<String>) = db
            .conn()
            .query_row(
                "SELECT chat_identifier, display_name FROM chat WHERE ROWID = ?",
                [chat_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(identifier, "iMessage;-;+15551234567");
        assert_eq!(name, Some("Alice".to_string()));
    }

    #[test]
    fn test_message_builder() {
        let mut db = TestIMessageDb::new().unwrap();
        let handle_id = db.handle(HandleBuilder::new("+15551234567")).unwrap();
        let chat_id = db
            .chat(ChatBuilder::new("iMessage;-;+15551234567"))
            .unwrap();

        let msg_id = db
            .message(
                MessageBuilder::new()
                    .text("Hello world!")
                    .handle(handle_id)
                    .chat(chat_id)
                    .date(12345),
            )
            .unwrap();

        let (text, hid, date): (String, i32, i64) = db
            .conn()
            .query_row(
                "SELECT text, handle_id, date FROM message WHERE ROWID = ?",
                [msg_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(text, "Hello world!");
        assert_eq!(hid, handle_id);
        assert_eq!(date, 12345);
    }

    #[test]
    fn test_addressbook_db() {
        let mut db = TestAddressBookDb::new().unwrap();
        let contact_id = db
            .contact(
                ContactBuilder::new()
                    .first_name("Alice")
                    .last_name("Johnson")
                    .phone("+15551234567")
                    .email("alice@example.com"),
            )
            .unwrap();

        let (first, last): (Option<String>, Option<String>) = db
            .conn()
            .query_row(
                "SELECT ZFIRSTNAME, ZLASTNAME FROM ZABCDRECORD WHERE Z_PK = ?",
                [contact_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(first, Some("Alice".to_string()));
        assert_eq!(last, Some("Johnson".to_string()));

        let phone: String = db
            .conn()
            .query_row(
                "SELECT ZFULLNUMBER FROM ZABCDPHONENUMBER WHERE ZOWNER = ?",
                [contact_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(phone, "+15551234567");
    }

    #[test]
    fn test_standard_scenario() {
        let (imessage_db, contacts_db) = standard_test_scenario().unwrap();

        let handle_count: i32 = imessage_db
            .conn()
            .query_row("SELECT COUNT(*) FROM handle", [], |row| row.get(0))
            .unwrap();
        assert_eq!(handle_count, 4);

        let chat_count: i32 = imessage_db
            .conn()
            .query_row("SELECT COUNT(*) FROM chat", [], |row| row.get(0))
            .unwrap();
        assert_eq!(chat_count, 5);

        let contact_count: i32 = contacts_db
            .conn()
            .query_row("SELECT COUNT(*) FROM ZABCDRECORD", [], |row| row.get(0))
            .unwrap();
        assert_eq!(contact_count, 3);
    }
}
