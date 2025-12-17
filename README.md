# ChatToMap Desktop

Desktop app for exporting iMessage chats to [ChatToMap.com](https://chattomap.com).

## Overview

ChatToMap Desktop uses [imessage-exporter](https://github.com/ReagentX/imessage-exporter) to read your iMessage database and export conversations for processing by ChatToMap. The app provides a simple UI for selecting chats and uploading them securely.

## Requirements

- **macOS**: Requires Full Disk Access permission to read `~/Library/Messages/chat.db`
- **Windows/Linux**: Requires an iTunes backup containing iMessage data

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

# Run development server
task dev
```

### Commands

| Command | Description |
|---------|-------------|
| `task dev` | Start Tauri development mode |
| `task build` | Build for production |
| `task ci` | Run all CI checks (required before commits) |
| `task lint` | Run Biome linter |
| `task lint:rust` | Run Clippy (Rust linter) |
| `task test` | Run all tests |
| `task typecheck` | TypeScript type checking |

### Quality Standards

This project follows strict quality standards:

- **Zero code duplication** (jscpd)
- **No `any` types** in TypeScript
- **No `biome-ignore` comments**
- **File length limits**: 500 lines (code), 1000 lines (tests)
- **Cognitive complexity**: max 15

All checks must pass before committing. Git hooks enforce this automatically.

## Architecture

```
chat_to_map_desktop/
├── src/                    # Frontend (TypeScript)
│   ├── index.html          # Main HTML
│   ├── main.ts             # Frontend logic
│   └── styles.css          # Styling
├── src-tauri/              # Backend (Rust)
│   ├── src/main.rs         # Tauri commands
│   ├── Cargo.toml          # Rust dependencies
│   └── tauri.conf.json     # Tauri configuration
└── Taskfile.yml            # Build commands
```

## License

GPL-3.0 (required due to bundling GPL-licensed imessage-exporter)
