//! Indexer module - orchestrates symbol extraction and database updates

pub mod database;
pub mod symbol;

pub use database::{Database, IndexStats};
pub use symbol::{
    Call, Dependency, DependencyKind, FileMetadata, FileStatus, Implementation, Symbol,
    SymbolKind, Usage, UsageKind, Visibility,
};

use crate::error::{CodeIndexError, Result};
use crate::parser::ParserFactory;
use log::{debug, info, warn};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Default patterns to ignore when indexing
const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    ".hg",
    ".svn",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    "dist",
    "build",
    ".next",
    ".nuxt",
    "vendor",
    ".cargo",
    "coverage",
];

/// Indexer orchestrates the parsing and database operations
pub struct Indexer {
    db: Database,
    ignore_patterns: Vec<String>,
}

impl Indexer {
    /// Create a new indexer with the given database path
    pub fn new(db_path: &Path) -> Result<Self> {
        let db = Database::new(db_path)?;
        Ok(Self {
            db,
            ignore_patterns: DEFAULT_IGNORE_PATTERNS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        })
    }

    /// Create an indexer from an existing database connection
    pub fn from_database(db: Database) -> Self {
        Self {
            db,
            ignore_patterns: DEFAULT_IGNORE_PATTERNS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    /// Get a reference to the underlying database
    pub fn database(&self) -> &Database {
        &self.db
    }

    /// Get a mutable reference to the underlying database
    pub fn database_mut(&mut self) -> &mut Database {
        &mut self.db
    }

    /// Add custom ignore patterns
    pub fn add_ignore_patterns(&mut self, patterns: &[String]) {
        self.ignore_patterns.extend(patterns.iter().cloned());
    }

    /// Index an entire directory recursively
    pub fn index_directory(&mut self, dir: &Path) -> Result<DirectoryIndexStats> {
        info!("Indexing directory: {}", dir.display());

        let mut stats = DirectoryIndexStats::default();

        // Collect files first to avoid borrow checker issues
        let ignore_patterns = self.ignore_patterns.clone();
        let files_to_index: Vec<_> = WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !Self::should_ignore_path(e.path(), &ignore_patterns))
            .filter_map(|entry| {
                match entry {
                    Ok(e) if e.file_type().is_file() => {
                        let path = e.path().to_path_buf();
                        if ParserFactory::for_file(&path).is_some() {
                            Some(Ok(path))
                        } else {
                            debug!("Skipping unsupported file: {}", path.display());
                            None
                        }
                    }
                    Ok(_) => None,
                    Err(e) => {
                        warn!("Error walking directory: {}", e);
                        Some(Err(e))
                    }
                }
            })
            .collect();

        // Now index each file
        for file_result in files_to_index {
            match file_result {
                Ok(path) => {
                    match self.index_file(&path) {
                        Ok(file_stats) => {
                            stats.files_indexed += 1;
                            stats.symbols_found += file_stats.symbols;
                            stats.dependencies_found += file_stats.dependencies;
                            stats.calls_found += file_stats.calls;
                            stats.implementations_found += file_stats.implementations;
                            stats.usages_found += file_stats.usages;
                        }
                        Err(e) => {
                            warn!("Error indexing file {}: {}", path.display(), e);
                            stats.errors += 1;
                        }
                    }
                }
                Err(_) => {
                    stats.errors += 1;
                }
            }
        }

        info!(
            "Indexing complete: {} files, {} symbols, {} deps, {} calls, {} impls, {} usages, {} errors",
            stats.files_indexed, stats.symbols_found, stats.dependencies_found,
            stats.calls_found, stats.implementations_found, stats.usages_found, stats.errors
        );

        Ok(stats)
    }

    /// Static helper to check if a path should be ignored
    fn should_ignore_path(path: &Path, patterns: &[String]) -> bool {
        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_string_lossy();
                for pattern in patterns {
                    if name_str == *pattern {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Index a single file
    pub fn index_file(&mut self, path: &Path) -> Result<FileIndexStats> {
        debug!("Indexing file: {}", path.display());

        let path = path.canonicalize().map_err(CodeIndexError::Io)?;

        // Get parser for this file type
        let parser = ParserFactory::for_file(&path)
            .ok_or_else(|| CodeIndexError::UnsupportedLanguage(path.display().to_string()))?;

        // Read file contents
        let source = fs::read_to_string(&path)?;

        // Parse all relationships
        let parse_result = parser.parse_all(&source, &path)?;

        // Get file metadata
        let metadata = fs::metadata(&path)?;
        let last_modified = metadata
            .modified()
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let lines_of_code = source.lines().count();

        // Create file metadata
        let mut file_meta = FileMetadata::new(&path, metadata.len(), last_modified)
            .with_language(parser.language_name())
            .with_lines_of_code(lines_of_code);

        // Update change count from existing record if present
        if let Ok(Some(existing)) = self.db.get_file(&path) {
            file_meta.change_count = existing.change_count + 1;
        }
        file_meta.calculate_hotness();

        // Clear existing data for this file
        self.db.delete_file_data(&path)?;

        // Insert new data
        self.db.upsert_file(&file_meta)?;

        for symbol in &parse_result.symbols {
            self.db.insert_symbol(symbol)?;
        }

        for dep in &parse_result.dependencies {
            self.db.insert_dependency(dep)?;
        }

        for call in &parse_result.calls {
            self.db.insert_call(call)?;
        }

        for impl_rel in &parse_result.implementations {
            self.db.insert_implementation(impl_rel)?;
        }

        for usage in &parse_result.usages {
            self.db.insert_usage(usage)?;
        }

        let stats = FileIndexStats {
            symbols: parse_result.symbols.len(),
            dependencies: parse_result.dependencies.len(),
            calls: parse_result.calls.len(),
            implementations: parse_result.implementations.len(),
            usages: parse_result.usages.len(),
        };

        debug!(
            "Indexed {}: {} symbols, {} deps, {} calls, {} impls, {} usages",
            path.display(),
            stats.symbols,
            stats.dependencies,
            stats.calls,
            stats.implementations,
            stats.usages
        );

        Ok(stats)
    }

    /// Remove a file from the index (mark as deleted)
    pub fn remove_file(&mut self, path: &Path) -> Result<()> {
        info!("Removing file from index: {}", path.display());
        self.db.mark_file_deleted(path)?;
        self.db.delete_file_symbols(path)?;
        self.db.delete_file_dependencies(path)?;
        Ok(())
    }

    /// Clear the entire index
    pub fn clear(&mut self) -> Result<()> {
        info!("Clearing entire index");
        self.db.clear_all()?;
        Ok(())
    }
}

/// Statistics from indexing a single file
#[derive(Debug, Default)]
pub struct FileIndexStats {
    pub symbols: usize,
    pub dependencies: usize,
    pub calls: usize,
    pub implementations: usize,
    pub usages: usize,
}

/// Statistics from indexing a directory
#[derive(Debug, Default)]
pub struct DirectoryIndexStats {
    pub files_indexed: usize,
    pub symbols_found: usize,
    pub dependencies_found: usize,
    pub calls_found: usize,
    pub implementations_found: usize,
    pub usages_found: usize,
    pub errors: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_should_ignore_path() {
        let patterns: Vec<String> = DEFAULT_IGNORE_PATTERNS.iter().map(|s| s.to_string()).collect();

        assert!(Indexer::should_ignore_path(Path::new("/foo/node_modules/bar.js"), &patterns));
        assert!(Indexer::should_ignore_path(Path::new("/foo/target/debug/main"), &patterns));
        assert!(Indexer::should_ignore_path(Path::new("/foo/.git/config"), &patterns));
        assert!(!Indexer::should_ignore_path(Path::new("/foo/src/main.rs"), &patterns));
        assert!(!Indexer::should_ignore_path(Path::new("/foo/lib/utils.ts"), &patterns));
    }

    #[test]
    fn test_indexer_creation() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let indexer = Indexer::new(&db_path).unwrap();

        // Database should exist
        assert!(db_path.exists());
    }
}
