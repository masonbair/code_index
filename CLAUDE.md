# code-index - Persistent Semantic Cache for AI Agents

## Project Overview

A background daemon that maintains a semantic index of codebases - symbol tables, dependency graphs, and file metadata. Acts as the foundational data layer for AI agent tooling.

**Primary Languages:** Rust
**Project Type:** System daemon/CLI tool
**Created:** 2026-04-21
**Target Platform:** Arch Linux (portable to other Linux distros)

---

## Purpose & Problem Statement

### Core Problem Solved
AI agents waste significant context tokens by re-parsing code structure every session. A persistent, incremental index provides instant access to code structure, eliminating redundant AST parsing and reducing context window usage by 70-80%.

### Key Value Propositions
- **Persistent cache**: Symbol tables survive across agent sessions
- **Incremental updates**: Uses inotify to track file changes in real-time
- **Language-agnostic**: Supports 66+ languages via tree-sitter
- **Fast queries**: Sub-100ms lookups via SQLite indexes
- **Foundation for other tools**: Used by code-summarizer, context-query, and context-packer

---

## Required Features

### 1. Symbol Indexing
- Extract functions, classes, types, interfaces, variables from source files
- Store full signatures with parameter types and return types
- Track line ranges for precise location mapping
- Support multiple programming languages (start with: Rust, TypeScript, Python, Go, JavaScript)

### 2. Dependency Tracking
- Parse import/require/use statements
- Build call hierarchy graphs (which functions call which)
- Track inheritance relationships for OOP languages
- Map module dependencies

### 3. File Metadata
- Track file size, last modified timestamp
- Calculate "hotness" score (combination of change frequency + cyclomatic complexity)
- Record change count over time
- Identify frequently-modified files for AI prioritization

### 4. Incremental Watching
- Use inotify (Linux) to watch file system changes
- Debounce rapid changes (e.g., during save operations)
- Incrementally re-index only changed files
- Handle file moves, renames, deletions

### 5. Query API
- JSON-based query interface for programmatic access
- Unix socket for inter-process communication (daemon mode)
- CLI interface for direct human/agent usage
- Support queries: by symbol name, by file, by dependency, by "hotness"

---

## Architecture

### High-Level Flow
```
┌─────────────────┐
│  File Watcher   │ (inotify via notify crate)
└────────┬────────┘
         │ File change events
         ▼
┌─────────────────┐
│   AST Parser    │ (tree-sitter for multi-language support)
└────────┬────────┘
         │ Extracted symbols & dependencies
         ▼
┌─────────────────┐
│  SQLite Index   │ (persistent storage with indexes)
└────────┬────────┘
         │ Query interface
         ▼
┌─────────────────┐
│   Query API     │ (JSON output, Unix socket, CLI)
└─────────────────┘
```

### Component Breakdown

**Daemon Process:**
- Runs in background via systemd user service (optional)
- Watches configured directories
- Maintains connection to SQLite database
- Exposes Unix socket at `~/.cache/ai-tools/code-index.sock`

**CLI Tool:**
- Starts/stops daemon
- Queries index via daemon socket or direct DB access
- Manual re-indexing commands
- Status reporting

---

## Database Schema (SQLite)

Store index at: `~/.cache/ai-tools/code-index.db`

```sql
-- Symbols table: Functions, classes, types, etc.
CREATE TABLE symbols (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,  -- 'function', 'class', 'struct', 'interface', 'type', 'variable', 'constant'
    file_path TEXT NOT NULL,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    signature TEXT,      -- Full signature for functions/methods
    parent_symbol TEXT,  -- For methods inside classes, nested functions
    visibility TEXT,     -- 'public', 'private', 'protected', etc.
    language TEXT NOT NULL,  -- 'rust', 'typescript', 'python', etc.
    indexed_at INTEGER NOT NULL,  -- Unix timestamp
    UNIQUE(name, file_path, line_start)
);

-- Dependencies table: Imports, calls, inheritance
CREATE TABLE dependencies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_file TEXT NOT NULL,
    target_file TEXT,     -- NULL for external dependencies
    kind TEXT NOT NULL,   -- 'import', 'call', 'inheritance', 'type_reference'
    symbol_name TEXT,
    line_number INTEGER,
    indexed_at INTEGER NOT NULL
);

-- Files metadata table
CREATE TABLE files (
    path TEXT PRIMARY KEY,
    size INTEGER NOT NULL,
    last_modified INTEGER NOT NULL,  -- Unix timestamp
    change_count INTEGER DEFAULT 0,  -- Incremented on each re-index
    hotness_score REAL DEFAULT 0.0,  -- Computed: change_count * complexity_factor
    language TEXT,
    lines_of_code INTEGER,
    indexed_at INTEGER NOT NULL,
    status TEXT DEFAULT 'indexed'    -- 'indexed', 'deleted', 'error'
);

-- Project configuration table
CREATE TABLE project_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Performance indexes
CREATE INDEX idx_symbols_name ON symbols(name);
CREATE INDEX idx_symbols_file ON symbols(file_path);
CREATE INDEX idx_symbols_kind ON symbols(kind);
CREATE INDEX idx_deps_source ON dependencies(source_file);
CREATE INDEX idx_deps_target ON dependencies(target_file);
CREATE INDEX idx_deps_symbol ON dependencies(symbol_name);
CREATE INDEX idx_files_hotness ON files(hotness_score DESC);
```

---

## CLI Interface Specification

### Command Structure
```bash
code-index <SUBCOMMAND> [OPTIONS]
```

### Subcommands

#### 1. Daemon Management
```bash
# Start daemon (watches current directory by default)
code-index daemon start [--watch <PATH>]

# Start daemon in foreground (for debugging)
code-index daemon start --foreground

# Stop daemon
code-index daemon stop

# Restart daemon
code-index daemon restart

# Check daemon status
code-index daemon status
# Output: Running | Stopped | Error
# Shows: PID, watched directories, indexed files count, last update time
```

#### 2. Indexing Operations
```bash
# Index a directory (one-time, no watching)
code-index index <PATH>

# Re-index everything from scratch
code-index reindex [--watch <PATH>]

# Force re-index specific file
code-index reindex --file <PATH>
```

#### 3. Query Operations
```bash
# Find symbol by name
code-index query --symbol "authenticateUser"
# Output (JSON):
# {
#   "results": [{
#     "file": "src/auth.ts",
#     "line": 42,
#     "kind": "function",
#     "signature": "async function authenticateUser(credentials: Credentials): Promise<User>"
#   }]
# }

# Get all symbols in a file
code-index query --file "src/auth.ts"

# Get dependencies of a file
code-index query --dependencies "src/auth/login.ts"
# Output: Lists imports and files that import this one

# Get call graph for a symbol
code-index query --symbol "login" --show-callers --depth 2

# Get hot files (frequently changed/complex)
code-index query --hot-files --limit 10
# Output: Top 10 files by hotness score

# List all symbols of a specific kind
code-index query --kind "function" --limit 50
```

#### 4. Statistics & Debugging
```bash
# Show statistics
code-index stats
# Output:
# - Total files indexed: 247
# - Total symbols: 1,834
# - Languages: TypeScript (68%), Rust (32%)
# - Database size: 2.4 MB
# - Last update: 2 minutes ago

# Export index to JSON (for debugging/backup)
code-index export --output index-backup.json

# Import index from JSON
code-index import --input index-backup.json

# Clear entire index
code-index clear [--confirm]
```

### Output Formats
- Default: Human-readable text
- `--json`: Machine-parsable JSON (for AI agents and scripts)
- `--format=table`: ASCII table format
- `--format=compact`: Minimal output (file:line only)

### Global Options
```bash
--db-path <PATH>     # Override default database location
--verbose, -v        # Verbose logging
--quiet, -q          # Suppress all non-error output
--help, -h           # Show help
--version, -V        # Show version
```

---

## Rust Implementation Guide

### Recommended Crate Dependencies

Add to `Cargo.toml`:
```toml
[dependencies]
# Core functionality
tree-sitter = "0.20"           # AST parsing
tree-sitter-rust = "0.20"      # Rust grammar
tree-sitter-typescript = "0.20" # TypeScript grammar
tree-sitter-python = "0.20"    # Python grammar
tree-sitter-go = "0.20"        # Go grammar
rusqlite = { version = "0.31", features = ["bundled"] }  # SQLite
notify = "6.1"                 # File system watching (inotify)
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"             # JSON serialization
tokio = { version = "1.36", features = ["full"] }  # Async runtime
clap = { version = "4.5", features = ["derive"] }  # CLI parsing
anyhow = "1.0"                 # Error handling
thiserror = "1.0"              # Custom errors
walkdir = "2.4"                # Directory traversal
log = "0.4"                    # Logging facade
env_logger = "0.11"            # Logging implementation
daemonize = "0.5"              # Daemon process creation (Linux)

[dev-dependencies]
tempfile = "3.10"              # Temp directories for tests
assert_cmd = "2.0"             # CLI testing
predicates = "3.1"             # Test assertions
```

### Module Structure

```
src/
├── main.rs              # CLI entry point, command routing
├── daemon.rs            # Daemon process management
├── watcher.rs           # File system watching (inotify wrapper)
├── parser/
│   ├── mod.rs           # Parser trait and factory
│   ├── rust.rs          # Rust-specific parsing
│   ├── typescript.rs    # TypeScript parsing
│   ├── python.rs        # Python parsing
│   └── common.rs        # Shared parsing utilities
├── indexer/
│   ├── mod.rs           # Indexer orchestration
│   ├── database.rs      # SQLite operations
│   ├── symbol.rs        # Symbol data structures
│   └── dependency.rs    # Dependency tracking
├── query/
│   ├── mod.rs           # Query API
│   ├── symbol_query.rs  # Symbol lookups
│   ├── dep_query.rs     # Dependency queries
│   └── file_query.rs    # File metadata queries
├── api/
│   ├── mod.rs           # API trait
│   ├── json.rs          # JSON output formatting
│   └── socket.rs        # Unix socket server (daemon mode)
├── config.rs            # Configuration management
├── error.rs             # Custom error types
└── utils.rs             # Utility functions (hotness calculation, etc.)

tests/
├── integration_test.rs  # End-to-end tests
├── parser_test.rs       # Parser tests
└── query_test.rs        # Query tests
```

### Key Implementation Details

#### 1. Parser Module (`src/parser/mod.rs`)
```rust
use tree_sitter::{Language, Parser, Tree};
use std::path::Path;

pub trait LanguageParser {
    fn parse_file(&self, path: &Path) -> anyhow::Result<Vec<Symbol>>;
    fn parse_dependencies(&self, path: &Path) -> anyhow::Result<Vec<Dependency>>;
}

pub struct ParserFactory;

impl ParserFactory {
    pub fn for_file(path: &Path) -> Option<Box<dyn LanguageParser>> {
        let ext = path.extension()?.to_str()?;
        match ext {
            "rs" => Some(Box::new(RustParser::new())),
            "ts" | "tsx" => Some(Box::new(TypeScriptParser::new())),
            "py" => Some(Box::new(PythonParser::new())),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: PathBuf,
    pub line_start: usize,
    pub line_end: usize,
    pub signature: Option<String>,
    pub parent: Option<String>,
    pub visibility: Visibility,
}

#[derive(Debug, Clone)]
pub enum SymbolKind {
    Function,
    Class,
    Struct,
    Interface,
    Type,
    Variable,
    Constant,
}
```

#### 2. Indexer Module (`src/indexer/database.rs`)
```rust
use rusqlite::{Connection, params};

pub struct Indexer {
    conn: Connection,
}

impl Indexer {
    pub fn new(db_path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(db_path)?;
        Self::create_schema(&conn)?;
        Ok(Self { conn })
    }

    fn create_schema(conn: &Connection) -> anyhow::Result<()> {
        conn.execute_batch(include_str!("../schema.sql"))?;
        Ok(())
    }

    pub fn insert_symbol(&mut self, symbol: &Symbol) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO symbols
             (name, kind, file_path, line_start, line_end, signature, language, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                symbol.name,
                symbol.kind.to_string(),
                symbol.file_path.to_str(),
                symbol.line_start,
                symbol.line_end,
                symbol.signature,
                symbol.language,
                chrono::Utc::now().timestamp(),
            ],
        )?;
        Ok(())
    }

    pub fn query_symbol(&self, name: &str) -> anyhow::Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM symbols WHERE name = ?1 ORDER BY file_path"
        )?;
        // Map rows to Symbol structs...
        Ok(symbols)
    }

    pub fn update_file_metadata(&mut self, file: &FileMetadata) -> anyhow::Result<()> {
        // Increment change_count, recalculate hotness_score
        self.conn.execute(
            "INSERT OR REPLACE INTO files
             (path, size, last_modified, change_count, hotness_score, indexed_at)
             VALUES (?1, ?2, ?3, COALESCE((SELECT change_count FROM files WHERE path = ?1), 0) + 1, ?4, ?5)",
            params![...],
        )?;
        Ok(())
    }
}
```

#### 3. File Watcher (`src/watcher.rs`)
```rust
use notify::{Watcher, RecommendedWatcher, RecursiveMode};
use std::sync::mpsc::channel;
use std::time::Duration;

pub struct FileWatcher {
    watcher: RecommendedWatcher,
}

impl FileWatcher {
    pub fn new(indexer: Arc<Mutex<Indexer>>) -> anyhow::Result<Self> {
        let (tx, rx) = channel();

        let watcher = RecommendedWatcher::new(tx, Duration::from_secs(2))?;

        // Spawn thread to handle events
        tokio::spawn(async move {
            for event in rx {
                if let Ok(event) = event {
                    Self::handle_event(event, &indexer).await;
                }
            }
        });

        Ok(Self { watcher })
    }

    pub fn watch(&mut self, path: &Path) -> anyhow::Result<()> {
        self.watcher.watch(path, RecursiveMode::Recursive)?;
        Ok(())
    }

    async fn handle_event(event: Event, indexer: &Arc<Mutex<Indexer>>) {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                // Re-index the file
                if let Some(path) = event.paths.first() {
                    Self::reindex_file(path, indexer).await;
                }
            }
            EventKind::Remove(_) => {
                // Mark file as deleted in DB
                if let Some(path) = event.paths.first() {
                    Self::mark_deleted(path, indexer).await;
                }
            }
            _ => {}
        }
    }
}
```

#### 4. Daemon Mode (`src/daemon.rs`)
```rust
use daemonize::Daemonize;
use std::fs::File;

pub fn run_daemon(config: Config) -> anyhow::Result<()> {
    let stdout = File::create("/tmp/code-index-daemon.out")?;
    let stderr = File::create("/tmp/code-index-daemon.err")?;

    let daemonize = Daemonize::new()
        .pid_file("/tmp/code-index.pid")
        .working_directory("/tmp")
        .stdout(stdout)
        .stderr(stderr);

    daemonize.start()?;

    // Main daemon loop
    let indexer = Arc::new(Mutex::new(Indexer::new(&config.db_path)?));
    let mut watcher = FileWatcher::new(indexer.clone())?;

    for path in config.watch_paths {
        watcher.watch(&path)?;
    }

    // Start Unix socket server for queries
    start_socket_server(indexer, &config.socket_path)?;

    // Keep running until SIGTERM
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

---

## Testing Requirements

### Unit Tests
- **Parser tests**: Verify symbol extraction for each language
- **Database tests**: CRUD operations, query correctness
- **Watcher tests**: Mock file system events
- **Hotness calculation**: Test scoring algorithm

### Integration Tests
```rust
#[test]
fn test_full_indexing_workflow() {
    // 1. Create temp directory with test code files
    let temp_dir = tempfile::tempdir().unwrap();
    create_test_files(&temp_dir);

    // 2. Initialize indexer
    let indexer = Indexer::new(&temp_dir.path().join("test.db")).unwrap();

    // 3. Index the directory
    index_directory(&temp_dir.path(), &indexer).unwrap();

    // 4. Query symbols
    let results = indexer.query_symbol("test_function").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].file_path, temp_dir.path().join("test.rs"));

    // 5. Test file watching
    // Modify a file, ensure it gets re-indexed
    // ...
}
```

### Performance Benchmarks
- Index 1000 files < 10 seconds
- Query response < 100ms
- Incremental update < 1 second per file

---

## Best Practices & Code Quality

### Error Handling
- Use `anyhow::Result` for functions that can fail
- Use `thiserror` for custom error types
- Never use `.unwrap()` in production code - handle all errors gracefully
- Log errors with context using `log::error!`

### Code Style
- Follow Rust standard naming conventions (snake_case for functions, PascalCase for types)
- Use `rustfmt` for formatting (include `rustfmt.toml`)
- Use `clippy` for linting - fix all warnings
- Add documentation comments (`///`) for public APIs
- Keep functions small and focused (< 50 lines ideal)

### Logging
- Use appropriate log levels:
  - `error!` - Critical failures
  - `warn!` - Recoverable issues
  - `info!` - High-level operations (indexing started/completed)
  - `debug!` - Detailed flow information
  - `trace!` - Very verbose debugging
- Include context in log messages: file paths, symbol names, counts

### Performance
- Use database transactions for batch inserts
- Create appropriate indexes (see schema above)
- Debounce file system events (2-second window)
- Use connection pooling if multi-threaded access needed

### Security
- Validate file paths (prevent directory traversal)
- Sanitize SQL inputs (use parameterized queries)
- Limit database size (implement pruning of old entries)
- Validate Unix socket permissions (0600)

---

## Git Workflow & Commits

### Branch Strategy
- `main` - Stable releases only
- `develop` - Active development
- Feature branches: `feature/<name>`
- Bug fixes: `fix/<issue>`

### Commit Messages
Follow conventional commits:
```
feat: Add TypeScript parser support
fix: Resolve race condition in file watcher
docs: Update CLI usage examples
test: Add integration tests for daemon mode
refactor: Simplify database query logic
perf: Optimize symbol lookup with better indexes
```

### Pre-commit Checklist
- [ ] Code compiles without warnings
- [ ] All tests pass (`cargo test`)
- [ ] Formatted with `cargo fmt`
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation updated if API changed
- [ ] Integration tests added for new features

---

## Configuration

Default config file: `~/.config/ai-tools/config.toml`

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
max_file_size_mb = 10  # Skip files larger than this
max_line_length = 2000 # Skip lines longer than this
debounce_delay_ms = 2000
```

---

## Installation & Packaging

### Build Instructions
```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Install to ~/.cargo/bin
cargo install --path .
```

### Systemd User Service (Optional)
Create `~/.config/systemd/user/code-index.service`:
```ini
[Unit]
Description=Code Index Daemon
After=network.target

[Service]
Type=simple
ExecStart=%h/.cargo/bin/code-index daemon start --foreground
Restart=on-failure

[Install]
WantedBy=default.target
```

Enable:
```bash
systemctl --user enable code-index
systemctl --user start code-index
```

---

## Success Criteria

- [ ] Successfully indexes Rust, TypeScript, Python, Go projects
- [ ] Daemon starts and watches directories in background
- [ ] Incremental updates work reliably (< 1s latency)
- [ ] Query API returns accurate results
- [ ] CLI interface is intuitive and well-documented
- [ ] All tests pass with >80% code coverage
- [ ] Performance benchmarks met (see Testing section)
- [ ] Ready for integration with other AI tools (code-summarizer, context-query, context-packer)

---

## Development Guidelines for AI Agents

**When working on this codebase:**

1. **Read `.ai/ARCHITECTURE.md` first** - Understand system structure
2. **Check `.ai/TOOLS.md`** for available development tools
3. **Follow `.ai/CONVENTIONS.md`** for code style and patterns
4. **Run tests before committing** - `cargo test`
5. **Update documentation** - Keep CLAUDE.md and `.ai/` files in sync with code
6. **Ask clarifying questions** - If requirements are unclear, ask before implementing
7. **Commit frequently** - Small, atomic commits with clear messages
8. **Think incrementally** - Build core features first, then optimize

---

## Next Steps After Implementation

Once code-index is built and tested:
1. Integrate with `ai-init` (optional flag: `ai-init myproject --start-index`)
2. Build `code-summarizer` (depends on code-index)
3. Build `context-query` (depends on code-index)
4. Build `context-packer` (depends on all above tools)

---

**Ready to build!** This spec provides everything needed to implement code-index. Focus on getting the core functionality working first (parsing, indexing, basic queries), then add daemon mode and advanced features.
