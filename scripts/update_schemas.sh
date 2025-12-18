#!/bin/bash
# Schema Update Script
# Dumps current database schemas and creates versioned files if changed
#
# Usage: ./scripts/update_schemas.sh
#
# This script:
# 1. Dumps schema from real iMessage and AddressBook databases
# 2. Compares against the latest versioned schema files
# 3. If changed: creates new versioned file (e.g., imessage_26.0.2.sql)
# 4. If unchanged: reports "no changes"

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/../src-tauri/src/test_fixtures"
SCHEMAS_DIR="$FIXTURES_DIR/schemas"

IMESSAGE_DB="$HOME/Library/Messages/chat.db"
ADDRESSBOOK_DB="$HOME/Library/Application Support/AddressBook/AddressBook-v22.abcddb"

# Get macOS version
MACOS_VERSION=$(sw_vers -productVersion)

# Create schemas directory if it doesn't exist
mkdir -p "$SCHEMAS_DIR"

# Function to dump schema from SQLite database
dump_schema() {
    local db_path="$1"
    local output_file="$2"
    local db_name="$3"

    if [[ ! -f "$db_path" ]]; then
        echo -e "${RED}Error: Database not found: $db_path${NC}"
        return 1
    fi

    # Create output with header
    {
        echo "-- $db_name Schema"
        echo "-- macOS Version: $MACOS_VERSION"
        echo "-- Dumped: $(date -u +"%Y-%m-%d %H:%M:%S UTC")"
        echo "-- Source: $db_path"
        echo ""
        echo "-- Tables"
        echo "-- ======"
        sqlite3 "$db_path" ".schema" | grep -E "^CREATE TABLE" | sort
        echo ""
        echo "-- Indexes"
        echo "-- ======="
        sqlite3 "$db_path" ".schema" | grep -E "^CREATE INDEX" | sort
    } > "$output_file"
}

# Function to normalize schema for comparison (strips comments and blank lines)
normalize_schema() {
    grep -E "^CREATE (TABLE|INDEX)" "$1" | sort
}

# Function to get latest versioned schema file
get_latest_schema() {
    local prefix="$1"
    local latest=""

    for f in "$SCHEMAS_DIR"/${prefix}_*.sql; do
        [[ -f "$f" ]] && latest="$f"
    done

    # Fall back to non-versioned file in parent directory
    if [[ -z "$latest" ]] && [[ -f "$FIXTURES_DIR/${prefix}_schema.sql" ]]; then
        latest="$FIXTURES_DIR/${prefix}_schema.sql"
    fi

    echo "$latest"
}

# Function to compare and update schema
update_schema() {
    local name="$1"
    local db_path="$2"
    local display_name="$3"
    local temp_file
    temp_file=$(mktemp)

    echo -e "${YELLOW}Processing $name...${NC}"

    # Dump current schema
    if ! dump_schema "$db_path" "$temp_file" "$display_name"; then
        rm -f "$temp_file"
        return 1
    fi

    # Get latest versioned schema
    local latest_schema
    latest_schema=$(get_latest_schema "$name")

    local new_schema_file="$SCHEMAS_DIR/${name}_${MACOS_VERSION}.sql"

    if [[ -z "$latest_schema" ]]; then
        # No existing schema - create first versioned file
        echo -e "${GREEN}Creating initial schema: ${new_schema_file}${NC}"
        mv "$temp_file" "$new_schema_file"
        return 0
    fi

    # Compare schemas (normalize both for comparison, stripping comments)
    if diff -q <(normalize_schema "$latest_schema") <(normalize_schema "$temp_file") > /dev/null 2>&1; then
        echo -e "${GREEN}$name: No schema changes from $(basename "$latest_schema")${NC}"
        rm -f "$temp_file"
    else
        echo -e "${YELLOW}$name: Schema changed! Creating ${new_schema_file}${NC}"

        # Show diff of normalized schemas
        echo -e "${YELLOW}Changes:${NC}"
        diff <(normalize_schema "$latest_schema") <(normalize_schema "$temp_file") || true

        mv "$temp_file" "$new_schema_file"
        echo -e "${GREEN}Created: ${new_schema_file}${NC}"
    fi
}

# Main
echo "Schema Update Script"
echo "===================="
echo "macOS Version: $MACOS_VERSION"
echo ""

# Check for Full Disk Access
if [[ ! -r "$IMESSAGE_DB" ]]; then
    echo -e "${RED}Error: Cannot read iMessage database.${NC}"
    echo "Please grant Full Disk Access to Terminal in System Preferences > Privacy & Security."
    exit 1
fi

# Update both schemas
update_schema "imessage" "$IMESSAGE_DB" "iMessage"
echo ""
update_schema "addressbook" "$ADDRESSBOOK_DB" "AddressBook"

echo ""
echo -e "${GREEN}Done!${NC}"
