# code-index - Architecture

**Status:** Active Development
**Last Updated:** 2026-04-30

---

## System Overview

code-index is a persistent semantic cache daemon for AI agents. It maintains an incremental index of codebases using tree-sitter for AST parsing and SQLite for storage, providing fast lookups for symbols, dependencies, call graphs, and more.

**Key Components:**
- **Parser Module**: Multi-language AST parsing using tree-sitter (Rust, TypeScript, Python)
- **Indexer Module**: Orchestrates parsing and database operations
- **Database Module**: SQLite storage with optimized schema and indexes
- **Query Module**: Query API for symbol/call/usage lookups
- **Daemon Module**: Background process management with file watching
- **Watcher Module**: inotify-based file system monitoring

---

## Directory Structure

```
code-index/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Library exports
│   ├── config.rs            # Configuration management
│   ├── daemon.rs            # Daemon process management
│   ├── error.rs             # Error types
│   ├── query.rs             # Query API
│   ├── watcher.rs           # File system watching
│   ├── schema.sql           # SQLite schema
│   ├── indexer/
│   │   ├── mod.rs           # Indexer orchestration
│   │   ├── database.rs      # SQLite operations
│   │   └── symbol.rs        # Data structures
│   └── parser/
│       ├── mod.rs           # Parser trait & factory
│       ├── rust.rs          # Rust parser
│       ├── python.rs        # Python parser
│       └── typescript.rs    # TypeScript/JavaScript parser
├── tests/
│   └── integration.rs       # Integration tests
└── .ai/                     # AI agent documentation
```

---

## Data Flow

```
┌─────────────────┐
│  File Watcher   │ (inotify via notify crate)
└────────┬────────┘
         │ File change events
         ▼
┌─────────────────┐
│   AST Parser    │ (tree-sitter for multi-language support)
└────────┬────────┘
         │ Extracted: symbols, dependencies, calls, implementations, usages
         ▼
┌─────────────────┐
│  SQLite Index   │ (persistent storage with indexes)
└────────┬────────┘
         │ Query interface
         ▼
┌─────────────────┐
│   Query API     │ (JSON output, CLI)
└─────────────────┘
```

---

## Database Schema

**Core Tables:**
- `symbols`: Functions, classes, structs, traits, etc.
- `dependencies`: Import/use statements
- `files`: File metadata and hotness scores
- `project_config`: Key-value configuration

**Relationship Tables (NEW):**
- `calls`: Function/method call relationships (caller -> callee)
- `implementations`: Trait implementation relationships (type -> trait)
- `usages`: Symbol reference tracking (where symbols are used)

---

## Key Decisions

### Tree-sitter for Parsing
- **Rationale:** Language-agnostic, incremental parsing, well-maintained grammars
- **Alternatives Considered:** rust-analyzer, Language Server Protocol
- **Trade-offs:** Limited semantic analysis vs. full compiler integration

### SQLite for Storage
- **Rationale:** Zero-dependency, fast, supports complex queries
- **Alternatives Considered:** rocksdb, sled, plain JSON
- **Trade-offs:** Single-writer limitation, but sufficient for daemon use case

### Daemon Architecture
- **Rationale:** Background indexing, instant query response
- **Alternatives Considered:** On-demand indexing only
- **Trade-offs:** Resource usage vs. latency

---

## Dependencies

### External Crates
- `tree-sitter` + language grammars: AST parsing
- `rusqlite`: SQLite database
- `notify`: File system watching
- `tokio`: Async runtime
- `clap`: CLI parsing
- `serde`/`serde_json`: Serialization

### Internal Dependencies
- Parser depends on tree-sitter grammars
- Indexer depends on Parser and Database
- Query depends on Database
- Daemon depends on Indexer, Watcher, Query

---

## For AI Agents

**Context Generation:** Run `code-index stats` for high-level overview.

**Quick Queries:**
- Symbol lookup: `code-index query symbol <name>`
- Find callers: `code-index query callers <function>`
- Find callees: `code-index query callees <function>`
- Find implementations: `code-index query implements <trait>`
- Find usages: `code-index query usages <symbol>`

**When to update:** After major architectural changes, new service integrations, or refactors.
