//! Custom error types for code-index

use thiserror::Error;

/// Main error type for code-index operations
#[derive(Error, Debug)]
pub enum CodeIndexError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error in {file}: {message}")]
    Parse { file: String, message: String },

    #[error("Unsupported language for file: {0}")]
    UnsupportedLanguage(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Daemon error: {0}")]
    Daemon(String),

    #[error("Watch error: {0}")]
    Watch(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

/// Result type alias for code-index operations
pub type Result<T> = std::result::Result<T, CodeIndexError>;
