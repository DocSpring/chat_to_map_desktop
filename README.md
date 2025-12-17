# ChatToMap Desktop

Desktop app for exporting iMessage chats to [ChatToMap.com](https://chattomap.com).

## Overview

ChatToMap Desktop reads your iMessage database directly and exports conversations for processing by ChatToMap. The app provides a simple UI for selecting chats and uploading them securely.

## Requirements

- **macOS only**: Requires Full Disk Access permission to read `~/Library/Messages/chat.db`

## Installation

Download the latest release from the [Releases](https://github.com/DocSpring/chat_to_map_desktop/releases) page.

On first launch, you'll be prompted to grant Full Disk Access in System Preferences.

---

## Development

### Prerequisites

- [Bun](https://bun.sh/) 1.3+
- [Rust](https://rustup.rs/) 1.70+
- [Task](https://taskfile.dev/) (task runner)

### Setup

```bash
# Install dependencies
bun install

# Install git hooks
task hooks:install
```

### Running the App

| Command | Description |
|---------|-------------|
| `task dev` | Development mode (uses **chattomap.com** API) |
| `task dev:local` | Development mode (uses **localhost:5173** API) |

For most development, use `task dev` which connects to the production API.

Use `task dev:local` only when testing against a local SaaS server.

### Building

| Command | Description |
|---------|-------------|
| `task build` | Production build (points to chattomap.com) |
| `task build:dev` | Release build pointing to localhost (for testing) |

### Testing

```bash
# Run all tests (Rust + TypeScript)
task test

# Run only Rust tests
task test:rust

# Run only TypeScript tests
task test:ts

# Run TypeScript tests in watch mode
task test:watch
```

### Quality Checks

```bash
# Run ALL checks (required before commits)
task ci
```

This runs: typecheck, lint, rust-lint, duplication check, file-length check, and all tests.

| Command | Description |
|---------|-------------|
| `task lint` | Biome linter (check only) |
| `task lint:fix` | Biome linter with auto-fix |
| `task lint:rust` | Clippy (Rust linter) |
| `task typecheck` | TypeScript type checking |
| `task duplication` | Check for code duplication |
| `task file-length` | Check file length limits |

### CLI Tool

A command-line tool is available for debugging and testing:

```bash
# Build the CLI
cd src-tauri && cargo build --bin ctm-cli

# List all chats
./target/debug/ctm-cli list-chats

# List chats with message counts
./target/debug/ctm-cli list-chats --show-counts

# Export specific chats (by ID)
./target/debug/ctm-cli export --chat-ids 1,5,12 --output export.zip
```

### Manual Testing Checklist

1. **Permission flow**: Launch app without Full Disk Access, verify permission screen appears
2. **Grant access**: Open System Preferences, grant FDA, click "Check Again"
3. **Chat list**: Verify chats load with correct names and message counts
4. **Selection**: Test select all/none, individual selection, filtering
5. **Export flow**: Select chats, click Export, verify progress stages
6. **Browser open**: Verify browser opens to results page on completion

---

## Architecture

```
chat_to_map_desktop/
├── src/                        # Frontend (TypeScript)
│   ├── index.html              # Main HTML
│   ├── main.ts                 # Frontend logic & state
│   ├── main.test.ts            # Frontend tests
│   └── styles.css              # Styling
├── src-tauri/                  # Backend (Rust)
│   ├── src/
│   │   ├── lib.rs              # Library exports
│   │   ├── main.rs             # Tauri commands (GUI)
│   │   ├── cli.rs              # CLI tool
│   │   ├── contacts.rs         # AddressBook integration
│   │   ├── export.rs           # Message export to JSON/zip
│   │   ├── upload.rs           # Server communication
│   │   └── test_fixtures.rs    # Test database builders
│   ├── Cargo.toml              # Rust dependencies
│   └── tauri.conf.json         # Tauri configuration
├── Taskfile.yml                # Build commands
└── reference/                  # Schema documentation
    └── imessage_schema.sql     # iMessage database schema
```

### Key Modules

| Module | Purpose |
|--------|---------|
| `contacts.rs` | Resolves phone/email to contact names via macOS AddressBook |
| `export.rs` | Reads iMessage DB, exports selected chats to JSON zip |
| `upload.rs` | Fetches pre-signed URLs, uploads to R2, creates processing jobs |

### Feature Flags

| Flag | Effect |
|------|--------|
| `dev-server` | Points to `localhost:5173` instead of `chattomap.com` |
| `desktop` | Enables Tauri GUI (default) |

---

## Quality Standards

This project follows strict quality standards:

- **Zero code duplication** (jscpd)
- **No `any` types** in TypeScript
- **No `biome-ignore` comments**
- **File length limits**: 500 lines (code), 1000 lines (tests)
- **Cognitive complexity**: max 15

All checks must pass before committing. Git hooks enforce this automatically.

## License

GPL-3.0 - See [LICENSE](LICENSE) for details.
