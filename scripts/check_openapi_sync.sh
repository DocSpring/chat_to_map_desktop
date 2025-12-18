#!/bin/bash
# Check if openapi.json is in sync with the SaaS app
#
# Usage:
#   ./scripts/check_openapi_sync.sh           # Check against local SaaS
#   ./scripts/check_openapi_sync.sh --update  # Update from local SaaS
#
# This script is meant to be run from the desktop app directory:
#   /path/to/chat_to_map_desktop/
#
# It expects the SaaS app to be at:
#   /path/to/chat_to_map_saas/static/openapi.json (after build)
#
# Or in a worktree setup:
#   ../static/openapi.json

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DESKTOP_ROOT="$(dirname "$SCRIPT_DIR")"
DESKTOP_SCHEMA="$DESKTOP_ROOT/src-tauri/openapi.json"

# Try to find the SaaS openapi.json
# 1. Worktree setup: ../static/openapi.json (desktop is inside saas worktree)
# 2. Sibling repo: ../chat_to_map_saas/static/openapi.json
SAAS_SCHEMA=""
if [[ -f "$DESKTOP_ROOT/../static/openapi.json" ]]; then
    SAAS_SCHEMA="$DESKTOP_ROOT/../static/openapi.json"
elif [[ -f "$DESKTOP_ROOT/../../chat_to_map_saas/static/openapi.json" ]]; then
    SAAS_SCHEMA="$DESKTOP_ROOT/../../chat_to_map_saas/static/openapi.json"
fi

check_sync() {
    if [[ ! -f "$DESKTOP_SCHEMA" ]]; then
        echo -e "${RED}Error: Desktop openapi.json not found at $DESKTOP_SCHEMA${NC}"
        echo "Run --update to copy it from the SaaS app"
        exit 1
    fi

    if [[ -z "$SAAS_SCHEMA" ]]; then
        echo -e "${YELLOW}Warning: SaaS openapi.json not found${NC}"
        echo "Expected at:"
        echo "  - ../static/openapi.json (worktree)"
        echo "  - ../../chat_to_map_saas/static/openapi.json (sibling)"
        echo ""
        echo "Build the SaaS app first: cd <saas> && bun run build"
        exit 1
    fi

    if ! diff -q "$DESKTOP_SCHEMA" "$SAAS_SCHEMA" > /dev/null 2>&1; then
        echo -e "${RED}Error: openapi.json is out of sync!${NC}"
        echo ""
        echo "Desktop: $DESKTOP_SCHEMA"
        echo "SaaS:    $SAAS_SCHEMA"
        echo ""
        echo "Run: ./scripts/check_openapi_sync.sh --update"
        exit 1
    fi

    echo -e "${GREEN}✅ openapi.json is in sync${NC}"
}

update_schema() {
    if [[ -z "$SAAS_SCHEMA" ]]; then
        echo -e "${RED}Error: SaaS openapi.json not found${NC}"
        echo "Build the SaaS app first: cd <saas> && bun run build"
        exit 1
    fi

    echo "Copying from: $SAAS_SCHEMA"
    echo "         to: $DESKTOP_SCHEMA"
    cp "$SAAS_SCHEMA" "$DESKTOP_SCHEMA"
    echo -e "${GREEN}✅ openapi.json updated${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. cargo build --lib  # Regenerate Rust types"
    echo "  2. cargo test         # Verify types compile"
    echo "  3. git add src-tauri/openapi.json"
    echo "  4. git commit -m 'Update openapi.json from SaaS'"
}

case "${1:-}" in
    --update|-u)
        update_schema
        ;;
    --help|-h)
        echo "Usage: $0 [--update]"
        echo ""
        echo "Check or update openapi.json sync with SaaS app."
        echo ""
        echo "Options:"
        echo "  --update, -u  Copy openapi.json from SaaS to desktop"
        echo "  --help, -h    Show this help"
        ;;
    *)
        check_sync
        ;;
esac
