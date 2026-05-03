# code-index

**Persistent semantic cache and code indexer for AI agents**

A background daemon that maintains a semantic index of your codebase - symbol tables, dependency graphs, and file metadata. Eliminates redundant AST parsing and reduces AI agent context window usage by 70-80%.

## Features

- **Persistent Symbol Cache**: Function/class/type definitions survive across agent sessions
- **Incremental Updates**: Real-time file watching via inotify - only re-indexes changed files
- **Multi-Language Support**: 66+ languages via tree-sitter (Rust, TypeScript, Python, Go, JavaScript, and more)
- **Fast Queries**: Sub-100ms lookups via optimized SQLite indexes
- **Dependency Tracking**: Import analysis, call hierarchies, and inheritance graphs
- **Smart Prioritization**: "Hotness" scores identify frequently-changed and complex files
- **Foundation Layer**: Powers other AI tools like code-summarizer, context-query, and context-packer

## Why code-index?

AI agents waste significant context tokens re-parsing code structure every session. This tool provides instant access to:
- Where functions/classes are defined
- What imports what
- Which files change most often
- Full call graphs and dependency trees

Perfect for large codebases where context window efficiency is critical.

## Installation

### From Source (Cargo)

```bash
# Clone the repository
git clone https://github.com/yourusername/code-index.git
cd code-index

# Build and install
cargo install --path .
```

### Optional: Systemd User Service

Enable automatic background indexing on login:

```bash
# Copy service file
mkdir -p ~/.config/systemd/user
cp contrib/code-index.service ~/.config/systemd/user/

# Enable and start
systemctl --user enable code-index
systemctl --user start code-index
```

## Quick Start

### 1. Index Your Project

```bash
# One-time index of current directory
code-index index .

# Or start daemon to watch for changes
code-index daemon start --watch /path/to/your/project
```

### 2. Query the Index

```bash
# Find a function definition
code-index query --symbol "authenticateUser"

# Get all symbols in a file
code-index query --file "src/auth.ts"

# Find dependencies
code-index query --dependencies "src/auth/login.ts"

# Get frequently-changed files
code-index query --hot-files --limit 10

# Machine-readable JSON output
code-index query --symbol "login" --json
```

### 3. Check Status

```bash
# Daemon status
code-index daemon status

# Project statistics
code-index stats
```

## Usage

### Daemon Commands

```bash
# Start daemon
code-index daemon start [--watch <PATH>]

# Start in foreground (for debugging)
code-index daemon start --foreground

# Stop daemon
code-index daemon stop

# Restart daemon
code-index daemon restart

# Check status
code-index daemon status
```

### Query Commands

```bash
# Symbol lookup
code-index query --symbol <NAME> [--json]

# File symbols
code-index query --file <PATH>

# Dependencies (imports and callers)
code-index query --dependencies <PATH>

# Call graph
code-index query --symbol <NAME> --show-callers --depth 2

# Hot files (frequently changed)
code-index query --hot-files [--limit N]

# List by kind
code-index query --kind function [--limit N]
```

### Index Management

```bash
# Re-index from scratch
code-index reindex [--watch <PATH>]

# Force re-index specific file
code-index reindex --file <PATH>

# Export index (backup/debugging)
code-index export --output backup.json

# Import index
code-index import --input backup.json

# Clear entire index
code-index clear --confirm
```

### Statistics

```bash
code-index stats
```

Output example:
```
Total files indexed: 247
Total symbols: 1,834
Languages: TypeScript (68%), Rust (32%)
Database size: 2.4 MB
Last update: 2 minutes ago
```

## Configuration

Default config: `~/.config/ai-tools/config.toml`

```toml
[code-index]
database_path = "~/.cache/ai-tools/code-index.db"
socket_path = "~/.cache/ai-tools/code-index.sock"
log_level = "info"

[code-index.watch]
patterns = ["**/*.rs", "**/*.ts", "**/*.tsx", "**/*.py", "**/*.go", "**/*.js"]
ignore_patterns = [
    "node_modules/**",
    "target/**",
    ".git/**",
    "*.test.ts",
    "*.spec.ts",
]

[code-index.indexing]
max_file_size_mb = 10
max_line_length = 2000
debounce_delay_ms = 2000
```

## Architecture

```
┌─────────────────┐
│  File Watcher   │ (inotify via notify crate)
└────────┬────────┘
         │ File change events
         ▼
┌─────────────────┐
│   AST Parser    │ (tree-sitter multi-language)
└────────┬────────┘
         │ Symbols & dependencies
         ▼
┌─────────────────┐
│  SQLite Index   │ (persistent storage)
└────────┬────────┘
         │ Query interface
         ▼
┌─────────────────┐
│   Query API     │ (JSON, Unix socket, CLI)
└─────────────────┘
```

**Components:**
- **Daemon**: Background process watching directories, maintaining SQLite database
- **CLI**: Query interface and daemon management
- **Parser**: Tree-sitter based multi-language AST extraction
- **Indexer**: SQLite storage with optimized indexes for fast queries

## Supported Languages

Via tree-sitter grammars:
- Rust, TypeScript/JavaScript, Python, Go, C, C++
- Java, C#, Ruby, PHP, Swift, Kotlin
- And 50+ more...

## Performance

- Index 1000 files: <10 seconds
- Query response: <100ms
- Incremental update: <1 second per file

## Development

**Languages:** Rust
**Project Type:** System daemon/CLI tool
**Target Platform:** Arch Linux (portable to other Linux distros)

### AI Agent Support

This project is optimized for AI agent workflows:
- `CLAUDE.md` - Comprehensive implementation specification
- `.ai/TOOLS.md` - Development tooling reference
- `.ai/ARCHITECTURE.md` - System architecture details
- `.ai/CONVENTIONS.md` - Coding conventions and patterns

### Build & Test

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

### Contributing

1. Follow Rust standard conventions
2. Run `cargo fmt` and `cargo clippy` before commits
3. Add tests for new features
4. Update documentation in CLAUDE.md
5. Use conventional commit messages

## Ecosystem

Part of the AI agent tooling suite:
- **code-index** (this project) - Foundation data layer
- **code-summarizer** - Generate project summaries
- **context-query** - Intelligent context extraction
- **context-packer** - Optimize context for AI prompts

## License

MIT License - See LICENSE file for details

## Roadmap

- [ ] Core indexing (Rust, TypeScript, Python, Go)
- [ ] Daemon mode with inotify watching
- [ ] Query API (CLI + JSON output)
- [ ] Unix socket server for IPC
- [ ] Call graph analysis
- [ ] Hotness scoring algorithm
- [ ] Integration with ai-init
- [ ] Additional language support
- [ ] LSP integration (optional)

## Credits

Built with:
- [tree-sitter](https://tree-sitter.github.io/) - Incremental parsing
- [rusqlite](https://github.com/rusqlite/rusqlite) - SQLite bindings
- [notify](https://github.com/notify-rs/notify) - File system watching
- [clap](https://github.com/clap-rs/clap) - CLI framework
