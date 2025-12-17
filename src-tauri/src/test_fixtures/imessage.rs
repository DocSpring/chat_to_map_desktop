/*!
 * iMessage database test fixtures
 */

use rusqlite::{Connection, Result};

/// Test iMessage database builder
pub struct TestIMessageDb {
    conn: Connection,
    next_handle_id: i32,
    next_chat_id: i32,
    next_message_id: i32,
}

impl TestIMessageDb {
    /// Create a new in-memory iMessage database with schema
    pub fn new() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn,
            next_handle_id: 1,
            next_chat_id: 1,
            next_message_id: 1,
        })
    }

    /// Initialize the database with minimal required tables
    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(include_str!("imessage_schema.sql"))?;
        Ok(())
    }

    /// Add a handle to the database
    pub fn handle(&mut self, builder: HandleBuilder) -> Result<i32> {
        let id = self.next_handle_id;
        self.next_handle_id += 1;

        self.conn.execute(
            "INSERT INTO handle (ROWID, id, country, service, uncanonicalized_id, person_centric_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                id,
                &builder.id,
                &builder.country,
                &builder.service,
                &builder.uncanonicalized_id,
                &builder.person_centric_id,
            ),
        )?;

        Ok(id)
    }

    /// Add a chat to the database
    pub fn chat(&mut self, builder: ChatBuilder) -> Result<i32> {
        let id = self.next_chat_id;
        self.next_chat_id += 1;

        let guid = builder
            .guid
            .unwrap_or_else(|| format!("chat-{}", builder.chat_identifier));

        self.conn.execute(
            "INSERT INTO chat (ROWID, guid, chat_identifier, service_name, display_name, style, room_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                id,
                &guid,
                &builder.chat_identifier,
                &builder.service_name,
                &builder.display_name,
                builder.style,
                &builder.room_name,
            ),
        )?;

        Ok(id)
    }

    /// Link a handle to a chat
    pub fn chat_handle(&mut self, chat_id: i32, handle_id: i32) -> Result<()> {
        self.conn.execute(
            "INSERT INTO chat_handle_join (chat_id, handle_id) VALUES (?1, ?2)",
            (chat_id, handle_id),
        )?;
        Ok(())
    }

    /// Add a message to the database
    pub fn message(&mut self, builder: MessageBuilder) -> Result<i32> {
        let id = self.next_message_id;
        self.next_message_id += 1;

        let guid = builder.guid.unwrap_or_else(|| format!("msg-{}", id));

        self.conn.execute(
            "INSERT INTO message (ROWID, guid, text, handle_id, service, date, is_from_me)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                id,
                &guid,
                &builder.text,
                builder.handle_id,
                &builder.service,
                builder.date,
                builder.is_from_me,
            ),
        )?;

        if let Some(chat_id) = builder.chat_id {
            self.conn.execute(
                "INSERT INTO chat_message_join (chat_id, message_id, message_date)
                 VALUES (?1, ?2, ?3)",
                (chat_id, id, builder.date),
            )?;
        }

        Ok(id)
    }

    /// Get the underlying connection for queries
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

impl Default for TestIMessageDb {
    fn default() -> Self {
        Self::new().expect("Failed to create test database")
    }
}

// =============================================================================
// Handle Builder
// =============================================================================

/// Builder for creating test handles
pub struct HandleBuilder {
    pub id: String,
    pub country: Option<String>,
    pub service: String,
    pub uncanonicalized_id: Option<String>,
    pub person_centric_id: Option<String>,
}

impl HandleBuilder {
    pub fn new<S: Into<String>>(id: S) -> Self {
        Self {
            id: id.into(),
            country: None,
            service: "iMessage".to_string(),
            uncanonicalized_id: None,
            person_centric_id: None,
        }
    }

    pub fn country<S: Into<String>>(mut self, country: S) -> Self {
        self.country = Some(country.into());
        self
    }

    pub fn service<S: Into<String>>(mut self, service: S) -> Self {
        self.service = service.into();
        self
    }

    #[allow(dead_code)]
    pub fn uncanonicalized<S: Into<String>>(mut self, id: S) -> Self {
        self.uncanonicalized_id = Some(id.into());
        self
    }
}

// =============================================================================
// Chat Builder
// =============================================================================

/// Builder for creating test chats
pub struct ChatBuilder {
    pub guid: Option<String>,
    pub chat_identifier: String,
    pub service_name: String,
    pub display_name: Option<String>,
    pub style: i32,
    pub room_name: Option<String>,
}

impl ChatBuilder {
    pub fn new<S: Into<String>>(chat_identifier: S) -> Self {
        Self {
            guid: None,
            chat_identifier: chat_identifier.into(),
            service_name: "iMessage".to_string(),
            display_name: None,
            style: 45,
            room_name: None,
        }
    }

    #[allow(dead_code)]
    pub fn guid<S: Into<String>>(mut self, guid: S) -> Self {
        self.guid = Some(guid.into());
        self
    }

    #[allow(dead_code)]
    pub fn service<S: Into<String>>(mut self, service: S) -> Self {
        self.service_name = service.into();
        self
    }

    pub fn display_name<S: Into<String>>(mut self, name: S) -> Self {
        self.display_name = Some(name.into());
        self
    }

    pub fn style(mut self, style: i32) -> Self {
        self.style = style;
        self
    }

    pub fn group(self) -> Self {
        self.style(43)
    }

    #[allow(dead_code)]
    pub fn room_name<S: Into<String>>(mut self, name: S) -> Self {
        self.room_name = Some(name.into());
        self
    }
}

// =============================================================================
// Message Builder
// =============================================================================

/// Builder for creating test messages
pub struct MessageBuilder {
    pub guid: Option<String>,
    pub text: Option<String>,
    pub handle_id: i32,
    pub service: String,
    pub date: i64,
    pub is_from_me: bool,
    pub chat_id: Option<i32>,
}

impl MessageBuilder {
    pub fn new() -> Self {
        Self {
            guid: None,
            text: None,
            handle_id: 0,
            service: "iMessage".to_string(),
            date: 0,
            is_from_me: false,
            chat_id: None,
        }
    }

    #[allow(dead_code)]
    pub fn guid<S: Into<String>>(mut self, guid: S) -> Self {
        self.guid = Some(guid.into());
        self
    }

    pub fn text<S: Into<String>>(mut self, text: S) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn handle(mut self, handle_id: i32) -> Self {
        self.handle_id = handle_id;
        self
    }

    #[allow(dead_code)]
    pub fn service<S: Into<String>>(mut self, service: S) -> Self {
        self.service = service.into();
        self
    }

    pub fn date(mut self, date: i64) -> Self {
        self.date = date;
        self
    }

    #[allow(dead_code)]
    pub fn from_me(mut self) -> Self {
        self.is_from_me = true;
        self
    }

    pub fn chat(mut self, chat_id: i32) -> Self {
        self.chat_id = Some(chat_id);
        self
    }
}

impl Default for MessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}
