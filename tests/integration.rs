//! Integration tests for code-index

use code_index::{Indexer, QueryEngine};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_full_indexing_workflow() {
    // Create temp directory with test code files
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create a test Rust file
    let rust_file = temp_dir.path().join("test.rs");
    fs::write(
        &rust_file,
        r#"
pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

pub struct User {
    name: String,
    age: u32,
}

impl User {
    pub fn new(name: String) -> Self {
        Self { name, age: 0 }
    }
}
"#,
    )
    .unwrap();

    // Create a test TypeScript file
    let ts_file = temp_dir.path().join("app.ts");
    fs::write(
        &ts_file,
        r#"
import { User } from "./models";

export function createUser(name: string): User {
    return { name, age: 0 };
}

export interface Config {
    host: string;
    port: number;
}
"#,
    )
    .unwrap();

    // Initialize indexer and index the directory
    let mut indexer = Indexer::new(&db_path).unwrap();
    let stats = indexer.index_directory(temp_dir.path()).unwrap();

    assert_eq!(stats.files_indexed, 2);
    assert!(stats.symbols_found > 0);
    assert_eq!(stats.errors, 0);

    // Query the database
    let db = indexer.database();
    let engine = QueryEngine::new(db);

    // Find the greet function
    let greet_symbols = engine.find_symbol("greet").unwrap();
    assert_eq!(greet_symbols.len(), 1);
    assert_eq!(greet_symbols[0].kind, code_index::SymbolKind::Function);

    // Find the User struct
    let user_symbols = engine.find_symbol("User").unwrap();
    assert!(user_symbols.len() >= 1);

    // Get all symbols in the Rust file
    let rust_symbols = engine.symbols_in_file(&rust_file).unwrap();
    assert!(rust_symbols.len() >= 3); // greet, User, new

    // Get hot files
    let hot_files = engine.hot_files(10).unwrap();
    assert_eq!(hot_files.len(), 2);
}

#[test]
fn test_incremental_reindex() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create initial file
    let rust_file = temp_dir.path().join("test.rs");
    fs::write(&rust_file, "pub fn foo() {}").unwrap();

    // Index
    let mut indexer = Indexer::new(&db_path).unwrap();
    indexer.index_file(&rust_file).unwrap();

    // Verify initial state
    {
        let db = indexer.database();
        let symbols = db.query_symbol_by_name("foo").unwrap();
        assert_eq!(symbols.len(), 1);

        // Get file metadata
        let file_meta = db.get_file(&rust_file.canonicalize().unwrap()).unwrap().unwrap();
        assert_eq!(file_meta.change_count, 1);
    }

    // Modify file
    fs::write(&rust_file, "pub fn foo() {}\npub fn bar() {}").unwrap();

    // Re-index
    indexer.index_file(&rust_file).unwrap();

    // Verify updated state
    let db = indexer.database();
    let symbols = db.query_symbol_by_name("bar").unwrap();
    assert_eq!(symbols.len(), 1);

    // Change count should be incremented
    let file_meta = db.get_file(&rust_file.canonicalize().unwrap()).unwrap().unwrap();
    assert_eq!(file_meta.change_count, 2);
}

#[test]
fn test_file_removal() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create and index a file
    let rust_file = temp_dir.path().join("test.rs");
    fs::write(&rust_file, "pub fn test_fn() {}").unwrap();

    let mut indexer = Indexer::new(&db_path).unwrap();
    indexer.index_file(&rust_file).unwrap();
    let canonical = rust_file.canonicalize().unwrap();

    // Verify it exists
    {
        let db = indexer.database();
        let symbols = db.query_symbol_by_name("test_fn").unwrap();
        assert_eq!(symbols.len(), 1);
    }

    // Remove the file from index
    indexer.remove_file(&canonical).unwrap();

    // Verify symbols are gone
    let db = indexer.database();
    let symbols = db.query_symbol_by_name("test_fn").unwrap();
    assert_eq!(symbols.len(), 0);

    // File metadata should be marked as deleted
    let file_meta = db.get_file(&canonical).unwrap().unwrap();
    assert_eq!(file_meta.status, code_index::FileStatus::Deleted);
}

#[test]
fn test_database_stats() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create test files
    fs::write(temp_dir.path().join("a.rs"), "fn a() {}").unwrap();
    fs::write(temp_dir.path().join("b.rs"), "fn b() {}").unwrap();
    fs::write(temp_dir.path().join("c.ts"), "function c() {}").unwrap();

    let mut indexer = Indexer::new(&db_path).unwrap();
    indexer.index_directory(temp_dir.path()).unwrap();

    let stats = indexer.database().get_stats().unwrap();

    assert_eq!(stats.total_files, 3);
    assert!(stats.total_symbols >= 3);
    assert!(stats.languages.len() >= 2); // rust and typescript
}

#[test]
fn test_clear_index() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create and index a file
    fs::write(temp_dir.path().join("test.rs"), "fn foo() {}").unwrap();

    let mut indexer = Indexer::new(&db_path).unwrap();
    indexer.index_directory(temp_dir.path()).unwrap();

    // Verify data exists
    let stats = indexer.database().get_stats().unwrap();
    assert!(stats.total_files > 0);

    // Clear
    indexer.clear().unwrap();

    // Verify cleared
    let stats = indexer.database().get_stats().unwrap();
    assert_eq!(stats.total_files, 0);
    assert_eq!(stats.total_symbols, 0);
}
