#!/bin/bash
# Update database schemas for test fixtures
# Compares current system schemas against stored versions
# Creates new versioned files if schemas have changed

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCHEMAS_DIR="$SCRIPT_DIR/../fixtures/schemas"
MACOS_VERSION=$(sw_vers -productVersion)

echo "macOS version: $MACOS_VERSION"
echo "Schemas directory: $SCHEMAS_DIR"
echo

# Temporary files for comparison
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

# iMessage database
IMESSAGE_DB="$HOME/Library/Messages/chat.db"
IMESSAGE_SCHEMA="$SCHEMAS_DIR/imessage_${MACOS_VERSION}.sql"
IMESSAGE_TMP="$TMP_DIR/imessage.sql"

echo "=== iMessage Schema ==="
if [ -f "$IMESSAGE_DB" ]; then
    sqlite3 "$IMESSAGE_DB" ".schema" > "$IMESSAGE_TMP" 2>/dev/null

    # Find latest existing schema
    LATEST_IMESSAGE=$(ls -v "$SCHEMAS_DIR"/imessage_*.sql 2>/dev/null | tail -1)

    if [ -z "$LATEST_IMESSAGE" ]; then
        echo "No existing schema found. Creating $IMESSAGE_SCHEMA"
        cp "$IMESSAGE_TMP" "$IMESSAGE_SCHEMA"
    elif [ -f "$IMESSAGE_SCHEMA" ]; then
        if diff -q "$IMESSAGE_TMP" "$IMESSAGE_SCHEMA" > /dev/null; then
            echo "No changes (already at $MACOS_VERSION)"
        else
            echo "Schema changed! Updating $IMESSAGE_SCHEMA"
            cp "$IMESSAGE_TMP" "$IMESSAGE_SCHEMA"
        fi
    else
        if diff -q "$IMESSAGE_TMP" "$LATEST_IMESSAGE" > /dev/null; then
            echo "No changes from $(basename "$LATEST_IMESSAGE")"
        else
            echo "New schema for $MACOS_VERSION (differs from $(basename "$LATEST_IMESSAGE"))"
            cp "$IMESSAGE_TMP" "$IMESSAGE_SCHEMA"
            echo "Created $IMESSAGE_SCHEMA"
        fi
    fi
else
    echo "iMessage database not found at $IMESSAGE_DB"
fi

echo

# AddressBook database
ADDRESSBOOK_DIR="$HOME/Library/Application Support/AddressBook/Sources"
ADDRESSBOOK_DB=$(find "$ADDRESSBOOK_DIR" -name "AddressBook-v22.abcddb" 2>/dev/null | head -1)
ADDRESSBOOK_SCHEMA="$SCHEMAS_DIR/addressbook_${MACOS_VERSION}.sql"
ADDRESSBOOK_TMP="$TMP_DIR/addressbook.sql"

echo "=== AddressBook Schema ==="
if [ -n "$ADDRESSBOOK_DB" ] && [ -f "$ADDRESSBOOK_DB" ]; then
    sqlite3 "$ADDRESSBOOK_DB" ".schema" > "$ADDRESSBOOK_TMP" 2>/dev/null

    # Find latest existing schema
    LATEST_ADDRESSBOOK=$(ls -v "$SCHEMAS_DIR"/addressbook_*.sql 2>/dev/null | tail -1)

    if [ -z "$LATEST_ADDRESSBOOK" ]; then
        echo "No existing schema found. Creating $ADDRESSBOOK_SCHEMA"
        cp "$ADDRESSBOOK_TMP" "$ADDRESSBOOK_SCHEMA"
    elif [ -f "$ADDRESSBOOK_SCHEMA" ]; then
        if diff -q "$ADDRESSBOOK_TMP" "$ADDRESSBOOK_SCHEMA" > /dev/null; then
            echo "No changes (already at $MACOS_VERSION)"
        else
            echo "Schema changed! Updating $ADDRESSBOOK_SCHEMA"
            cp "$ADDRESSBOOK_TMP" "$ADDRESSBOOK_SCHEMA"
        fi
    else
        if diff -q "$ADDRESSBOOK_TMP" "$LATEST_ADDRESSBOOK" > /dev/null; then
            echo "No changes from $(basename "$LATEST_ADDRESSBOOK")"
        else
            echo "New schema for $MACOS_VERSION (differs from $(basename "$LATEST_ADDRESSBOOK"))"
            cp "$ADDRESSBOOK_TMP" "$ADDRESSBOOK_SCHEMA"
            echo "Created $ADDRESSBOOK_SCHEMA"
        fi
    fi
else
    echo "AddressBook database not found"
fi

echo
echo "=== Current schemas ==="
ls -la "$SCHEMAS_DIR"/*.sql 2>/dev/null || echo "No schemas found"
