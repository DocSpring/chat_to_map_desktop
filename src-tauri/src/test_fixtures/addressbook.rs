/*!
 * AddressBook database test fixtures
 */

use rusqlite::{Connection, Result};

/// Test AddressBook database builder
pub struct TestAddressBookDb {
    conn: Connection,
    next_contact_id: i32,
    next_phone_id: i32,
    next_email_id: i32,
}

impl TestAddressBookDb {
    /// Create a new in-memory AddressBook database with schema
    pub fn new() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn,
            next_contact_id: 1,
            next_phone_id: 1,
            next_email_id: 1,
        })
    }

    /// Initialize the database with minimal required tables
    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(include_str!("addressbook_schema.sql"))?;
        Ok(())
    }

    /// Add a contact to the database
    pub fn contact(&mut self, builder: ContactBuilder) -> Result<i32> {
        let id = self.next_contact_id;
        self.next_contact_id += 1;

        self.conn.execute(
            "INSERT INTO ZABCDRECORD (Z_PK, Z_ENT, ZFIRSTNAME, ZLASTNAME, ZMIDDLENAME, ZNICKNAME, ZORGANIZATION)
             VALUES (?1, 19, ?2, ?3, ?4, ?5, ?6)",
            (
                id,
                &builder.first_name,
                &builder.last_name,
                &builder.middle_name,
                &builder.nickname,
                &builder.organization,
            ),
        )?;

        for phone in builder.phones {
            self.phone(id, phone)?;
        }

        for email in builder.emails {
            self.email(id, email)?;
        }

        Ok(id)
    }

    fn phone(&mut self, owner_id: i32, number: String) -> Result<i32> {
        let id = self.next_phone_id;
        self.next_phone_id += 1;

        self.conn.execute(
            "INSERT INTO ZABCDPHONENUMBER (Z_PK, ZOWNER, ZFULLNUMBER) VALUES (?1, ?2, ?3)",
            (id, owner_id, &number),
        )?;

        Ok(id)
    }

    fn email(&mut self, owner_id: i32, address: String) -> Result<i32> {
        let id = self.next_email_id;
        self.next_email_id += 1;

        let normalized = address.to_lowercase();
        self.conn.execute(
            "INSERT INTO ZABCDEMAILADDRESS (Z_PK, ZOWNER, ZADDRESS, ZADDRESSNORMALIZED)
             VALUES (?1, ?2, ?3, ?4)",
            (id, owner_id, &address, &normalized),
        )?;

        Ok(id)
    }

    /// Get the underlying connection for queries
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

impl Default for TestAddressBookDb {
    fn default() -> Self {
        Self::new().expect("Failed to create test database")
    }
}

// =============================================================================
// Contact Builder
// =============================================================================

/// Builder for creating test contacts
pub struct ContactBuilder {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub middle_name: Option<String>,
    pub nickname: Option<String>,
    pub organization: Option<String>,
    pub phones: Vec<String>,
    pub emails: Vec<String>,
}

impl ContactBuilder {
    pub fn new() -> Self {
        Self {
            first_name: None,
            last_name: None,
            middle_name: None,
            nickname: None,
            organization: None,
            phones: Vec::new(),
            emails: Vec::new(),
        }
    }

    pub fn first_name<S: Into<String>>(mut self, name: S) -> Self {
        self.first_name = Some(name.into());
        self
    }

    pub fn last_name<S: Into<String>>(mut self, name: S) -> Self {
        self.last_name = Some(name.into());
        self
    }

    #[allow(dead_code)]
    pub fn nickname<S: Into<String>>(mut self, name: S) -> Self {
        self.nickname = Some(name.into());
        self
    }

    #[allow(dead_code)]
    pub fn organization<S: Into<String>>(mut self, name: S) -> Self {
        self.organization = Some(name.into());
        self
    }

    pub fn phone<S: Into<String>>(mut self, number: S) -> Self {
        self.phones.push(number.into());
        self
    }

    pub fn email<S: Into<String>>(mut self, address: S) -> Self {
        self.emails.push(address.into());
        self
    }
}

impl Default for ContactBuilder {
    fn default() -> Self {
        Self::new()
    }
}
