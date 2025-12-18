#!/bin/bash
# Check file length limits
# - Code files (TS/RS): 500 lines max
# - Test files: 1000 lines max

set -e

MAX_CODE_LINES=500
MAX_TEST_LINES=1000
EXIT_CODE=0

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

check_file() {
  local file="$1"
  local line_count

  # Skip if file doesn't exist or is a symlink
  [[ ! -f "$file" ]] && return 0
  [[ -L "$file" ]] && return 0

  # Skip excluded patterns
  case "$file" in
    *.md|*.json|*.yml|*.yaml|*.toml|*.txt|*.lock|*.css|*.html|LICENSE|COPYING|CREDITS*)
      return 0
      ;;
    *.png|*.jpg|*.jpeg|*.gif|*.ico|*.icns|*.svg|*.webp)
      return 0
      ;;
    *node_modules*|*target/*|*dist/*|*build/*)
      return 0
      ;;
  esac

  line_count=$(wc -l < "$file" | tr -d ' ')

  # Determine max lines based on file type
  local max_lines=$MAX_CODE_LINES
  if [[ "$file" == *.test.ts ]] || [[ "$file" == *.spec.ts ]] || [[ "$file" == *_test.rs ]]; then
    max_lines=$MAX_TEST_LINES
  fi

  if [[ $line_count -gt $max_lines ]]; then
    echo -e "${RED}❌ $file: $line_count lines (max: $max_lines)${NC}"
    EXIT_CODE=1
  fi
}

# If specific files are passed, check only those
if [[ $# -gt 0 ]]; then
  for file in "$@"; do
    check_file "$file"
  done
else
  # Check all TypeScript files in src/
  while IFS= read -r -d '' file; do
    check_file "$file"
  done < <(find src -type f \( -name "*.ts" -o -name "*.tsx" -o -name "*.js" \) -print0 2>/dev/null || true)

  # Check all Rust files in src-tauri/
  while IFS= read -r -d '' file; do
    check_file "$file"
  done < <(find src-tauri -type f -name "*.rs" -print0 2>/dev/null || true)
fi

if [[ $EXIT_CODE -eq 0 ]]; then
  echo -e "${GREEN}✅ All files within length limits${NC}"
fi

exit $EXIT_CODE
