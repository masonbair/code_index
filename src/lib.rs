//! code-index: Persistent semantic cache for AI agents
//!
//! A background daemon that maintains a semantic index of codebases using
//! tree-sitter for AST parsing and SQLite for storage.

pub mod config;
pub mod daemon;
pub mod error;
pub mod indexer;
pub mod parser;
pub mod query;
pub mod watcher;

// Re-export commonly used types
pub use config::{Config, OutputFormat};
pub use daemon::{DaemonManager, DaemonStatus};
pub use error::{CodeIndexError, Result};
pub use indexer::{
    Call, Database, Dependency, DependencyKind, DirectoryIndexStats, FileMetadata, FileStatus,
    Implementation, IndexStats, Indexer, Symbol, SymbolKind, Usage, UsageKind, Visibility,
};
pub use parser::{LanguageParser, ParseResult, ParserFactory};
pub use query::QueryEngine;
pub use watcher::{FileWatcher, WatchManager};
