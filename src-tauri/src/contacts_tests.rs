/*!
 * Tests for contacts module
 */

use super::*;

// =============================================================================
// In-Memory Fixtures (for unit tests that don't need real DB)
// =============================================================================

/// Build a contacts index from in-memory fixtures
fn build_test_contacts_index() -> ContactsIndex {
    let mut index = HashMap::new();

    // Alice Johnson - US phone
    let alice = Name {
        first: "Alice".to_string(),
        last: "Johnson".to_string(),
        full: "Alice Johnson".to_string(),
        details: String::new(),
        handle_ids: HashSet::new(),
    };
    for key in phone_keys("+15551234567") {
        index.insert(key, alice.clone());
    }

    // Bob Williams - NZ phone
    let bob = Name {
        first: "Bob".to_string(),
        last: "Williams".to_string(),
        full: "Bob Williams".to_string(),
        details: String::new(),
        handle_ids: HashSet::new(),
    };
    for key in phone_keys("+6421555123") {
        index.insert(key, bob.clone());
    }

    // Charlie Brown - email
    let charlie = Name {
        first: "Charlie".to_string(),
        last: "Brown".to_string(),
        full: "Charlie Brown".to_string(),
        details: String::new(),
        handle_ids: HashSet::new(),
    };
    if let Some(normalized) = normalize_email("charlie@example.com") {
        index.insert(normalized, charlie);
    }

    ContactsIndex::from_index(index)
}

/// Fixture: iMessage handles (handle_id -> phone/email)
fn fixture_handles() -> HashMap<i32, String> {
    let mut handles = HashMap::new();
    handles.insert(0, "Me".to_string());
    handles.insert(81, "+15551234567".to_string());
    handles.insert(42, "+6421555123".to_string());
    handles.insert(99, "charlie@example.com".to_string());
    handles.insert(100, "+6421999888".to_string()); // Unknown
    handles
}

/// Fixture: Realistic dedup mapping (Handle::dedupe behavior)
fn fixture_deduped_handles_realistic() -> HashMap<i32, i32> {
    let mut deduped = HashMap::new();
    deduped.insert(0, 0);
    deduped.insert(42, 1);
    deduped.insert(81, 2);
    deduped.insert(99, 3);
    deduped.insert(100, 4);
    deduped
}

/// Fixture: Identity mapping (handle_id == deduped_id)
fn fixture_deduped_handles_identity() -> HashMap<i32, i32> {
    let mut deduped = HashMap::new();
    deduped.insert(0, 0);
    deduped.insert(81, 81);
    deduped.insert(42, 42);
    deduped.insert(99, 99);
    deduped.insert(100, 100);
    deduped
}

// =============================================================================
// Unit Tests: Phone Key Generation
// =============================================================================

#[test]
fn test_phone_keys_us_number_with_plus1() {
    let keys = phone_keys("+15551234567");
    assert!(keys.contains(&"15551234567".to_string()));
    assert!(keys.contains(&"+15551234567".to_string()));
    assert!(keys.contains(&"5551234567".to_string()));
    assert!(keys.contains(&"+5551234567".to_string()));
}

#[test]
fn test_phone_keys_nz_number() {
    let keys = phone_keys("+6421555123");
    assert!(keys.contains(&"6421555123".to_string()));
    assert!(keys.contains(&"+6421555123".to_string()));
    assert_eq!(keys.len(), 2);
}

#[test]
fn test_phone_keys_urn_skipped() {
    let keys = phone_keys("urn:biz:12345");
    assert!(keys.is_empty());
}

// =============================================================================
// Unit Tests: Contact Lookup
// =============================================================================

#[test]
fn test_lookup_us_phone_exact() {
    let index = build_test_contacts_index();
    let result = index.lookup("+15551234567");
    assert!(result.is_some());
    assert_eq!(result.unwrap().full, "Alice Johnson");
}

#[test]
fn test_lookup_us_phone_without_plus() {
    let index = build_test_contacts_index();
    let result = index.lookup("15551234567");
    assert!(result.is_some());
    assert_eq!(result.unwrap().full, "Alice Johnson");
}

#[test]
fn test_lookup_us_phone_last_10_digits() {
    let index = build_test_contacts_index();
    let result = index.lookup("5551234567");
    assert!(result.is_some());
    assert_eq!(result.unwrap().full, "Alice Johnson");
}

#[test]
fn test_lookup_nz_phone() {
    let index = build_test_contacts_index();
    let result = index.lookup("+6421555123");
    assert!(result.is_some());
    assert_eq!(result.unwrap().full, "Bob Williams");
}

#[test]
fn test_lookup_email() {
    let index = build_test_contacts_index();
    let result = index.lookup("charlie@example.com");
    assert!(result.is_some());
    assert_eq!(result.unwrap().full, "Charlie Brown");
}

#[test]
fn test_lookup_email_case_insensitive() {
    let index = build_test_contacts_index();
    let result = index.lookup("CHARLIE@EXAMPLE.COM");
    assert!(result.is_some());
    assert_eq!(result.unwrap().full, "Charlie Brown");
}

#[test]
fn test_lookup_unknown_returns_none() {
    let index = build_test_contacts_index();
    let result = index.lookup("+6421999888");
    assert!(result.is_none());
}

// =============================================================================
// Unit Tests: Participants Map Building
// =============================================================================

#[test]
fn test_build_participants_map_resolves_contact() {
    let contacts = build_test_contacts_index();
    let handles = fixture_handles();
    let deduped = fixture_deduped_handles_identity();

    let participants_map = contacts.build_participants_map(&handles, &deduped);

    let alice = participants_map.get(&81);
    assert!(alice.is_some());
    assert_eq!(alice.unwrap().full, "Alice Johnson");
}

#[test]
fn test_build_participants_map_unknown_falls_back_to_details() {
    let contacts = build_test_contacts_index();
    let handles = fixture_handles();
    let deduped = fixture_deduped_handles_identity();

    let participants_map = contacts.build_participants_map(&handles, &deduped);

    let unknown = participants_map.get(&100);
    assert!(unknown.is_some());
    assert_eq!(unknown.unwrap().details, "+6421999888");
    assert!(unknown.unwrap().full.is_empty());
}

// =============================================================================
// Unit Tests: Realistic Deduplication
// =============================================================================

/// Verify that participants_map is keyed by deduped_id, NOT handle_id.
#[test]
fn test_participants_map_keyed_by_deduped_id() {
    let contacts = build_test_contacts_index();
    let handles = fixture_handles();
    let deduped = fixture_deduped_handles_realistic();

    let participants_map = contacts.build_participants_map(&handles, &deduped);

    // Looking up by handle_id 81 returns None (map keyed by deduped_id)
    assert!(!participants_map.contains_key(&81));

    // Looking up by deduped_id 2 works
    let name = participants_map.get(&2);
    assert!(name.is_some());
    assert_eq!(name.unwrap().get_display_name(), "Alice Johnson");
}

/// Demonstrate correct lookup pattern: handle_id -> deduped_id -> name
#[test]
fn test_correct_lookup_pattern() {
    let contacts = build_test_contacts_index();
    let handles = fixture_handles();
    let deduped = fixture_deduped_handles_realistic();

    let participants_map = contacts.build_participants_map(&handles, &deduped);

    let handle_id = 81;
    let deduped_id = deduped.get(&handle_id).unwrap();
    assert_eq!(*deduped_id, 2);

    let name = participants_map.get(deduped_id);
    assert!(name.is_some());
    assert_eq!(name.unwrap().get_display_name(), "Alice Johnson");
}

// =============================================================================
// Integration Tests: Real SQLite Fixtures
// =============================================================================

mod integration {
    use super::*;
    use crate::test_fixtures::{ContactBuilder, TestAddressBookDb};

    #[test]
    fn test_build_from_real_macos_db() {
        let mut db = TestAddressBookDb::default();

        db.contact(
            ContactBuilder::new()
                .first_name("Alice")
                .last_name("Johnson")
                .phone("+15551234567"),
        )
        .unwrap();

        db.contact(
            ContactBuilder::new()
                .first_name("Bob")
                .last_name("Williams")
                .phone("+6421555123"),
        )
        .unwrap();

        db.contact(
            ContactBuilder::new()
                .first_name("Charlie")
                .last_name("Brown")
                .email("charlie@example.com"),
        )
        .unwrap();

        let index = ContactsIndex::build_from_macos(db.conn()).unwrap();

        assert!(index.lookup("+15551234567").is_some());
        assert!(index.lookup("+6421555123").is_some());
        assert!(index.lookup("charlie@example.com").is_some());
        assert!(index.lookup("+9999999999").is_none());
    }

    #[test]
    fn test_us_phone_variations_real_db() {
        let mut db = TestAddressBookDb::default();

        db.contact(
            ContactBuilder::new()
                .first_name("Alice")
                .last_name("Johnson")
                .phone("+15551234567"),
        )
        .unwrap();

        let index = ContactsIndex::build_from_macos(db.conn()).unwrap();

        assert!(index.lookup("+15551234567").is_some());
        assert!(index.lookup("15551234567").is_some());
        assert!(index.lookup("5551234567").is_some());
        assert!(index.lookup("+5551234567").is_some());
    }

    #[test]
    fn test_contact_multiple_phones_real_db() {
        let mut db = TestAddressBookDb::default();

        db.contact(
            ContactBuilder::new()
                .first_name("Alice")
                .last_name("Johnson")
                .phone("+15551234567")
                .phone("+15559876543"),
        )
        .unwrap();

        let index = ContactsIndex::build_from_macos(db.conn()).unwrap();

        let alice1 = index.lookup("+15551234567");
        let alice2 = index.lookup("+15559876543");

        assert!(alice1.is_some());
        assert!(alice2.is_some());
        assert_eq!(alice1.unwrap().full, "Alice Johnson");
        assert_eq!(alice2.unwrap().full, "Alice Johnson");
    }

    #[test]
    fn test_contact_phone_and_email_real_db() {
        let mut db = TestAddressBookDb::default();

        db.contact(
            ContactBuilder::new()
                .first_name("Alice")
                .last_name("Johnson")
                .phone("+15551234567")
                .email("alice@example.com"),
        )
        .unwrap();

        let index = ContactsIndex::build_from_macos(db.conn()).unwrap();

        assert_eq!(index.lookup("+15551234567").unwrap().full, "Alice Johnson");
        assert_eq!(
            index.lookup("alice@example.com").unwrap().full,
            "Alice Johnson"
        );
    }

    #[test]
    fn test_first_name_only_contact() {
        let mut db = TestAddressBookDb::default();
        db.contact(
            ContactBuilder::new()
                .first_name("Madonna")
                .phone("+15551234567"),
        )
        .unwrap();

        let index = ContactsIndex::build_from_macos(db.conn()).unwrap();
        assert_eq!(index.lookup("+15551234567").unwrap().full, "Madonna");
    }

    #[test]
    fn test_last_name_only_contact() {
        let mut db = TestAddressBookDb::default();
        db.contact(
            ContactBuilder::new()
                .last_name("Smith")
                .phone("+15551234567"),
        )
        .unwrap();

        let index = ContactsIndex::build_from_macos(db.conn()).unwrap();
        assert_eq!(index.lookup("+15551234567").unwrap().full, "Smith");
    }

    #[test]
    fn test_empty_contacts_db() {
        let db = TestAddressBookDb::default();
        let index = ContactsIndex::build_from_macos(db.conn()).unwrap();

        assert!(index.is_empty());
        assert!(index.lookup("+15551234567").is_none());
    }

    #[test]
    fn test_participants_map_with_real_db() {
        let mut contacts_db = TestAddressBookDb::default();

        contacts_db
            .contact(
                ContactBuilder::new()
                    .first_name("Alice")
                    .last_name("Johnson")
                    .phone("+15551234567"),
            )
            .unwrap();

        let index = ContactsIndex::build_from_macos(contacts_db.conn()).unwrap();

        let mut handles = HashMap::new();
        handles.insert(1, "+15551234567".to_string());
        handles.insert(2, "+9999999999".to_string());

        let mut deduped = HashMap::new();
        deduped.insert(1, 1);
        deduped.insert(2, 2);

        let participants_map = index.build_participants_map(&handles, &deduped);

        assert_eq!(participants_map.get(&1).unwrap().full, "Alice Johnson");
        assert_eq!(participants_map.get(&2).unwrap().details, "+9999999999");
    }
}
