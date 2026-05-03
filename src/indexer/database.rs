//! SQLite database operations for the code index

use crate::error::Result;
use crate::indexer::{
    Call, Dependency, DependencyKind, FileMetadata, FileStatus, Implementation, Symbol,
    SymbolKind, Usage, UsageKind, Visibility,
};
use log::{debug, info};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

/// Database wrapper for code index storage
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Create or open a database at the given path
    pub fn new(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Create an in-memory database (for testing)
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Initialize the database schema
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(include_str!("../schema.sql"))?;
        debug!("Database schema initialized");
        Ok(())
    }

    // ==================== Symbol Operations ====================

    /// Insert a symbol into the database
    pub fn insert_symbol(&self, symbol: &Symbol) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();

        self.conn.execute(
            "INSERT OR REPLACE INTO symbols
             (name, kind, file_path, line_start, line_end, signature, parent_symbol, visibility, language, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                symbol.name,
                symbol.kind.as_str(),
                symbol.file_path.to_string_lossy(),
                symbol.line_start,
                symbol.line_end,
                symbol.signature,
                symbol.parent,
                symbol.visibility.as_str(),
                symbol.language,
                now,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Query symbols by name (exact match)
    pub fn query_symbol_by_name(&self, name: &str) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, kind, file_path, line_start, line_end, signature, parent_symbol, visibility, language
             FROM symbols WHERE name = ?1 ORDER BY file_path",
        )?;

        let symbols = stmt
            .query_map(params![name], |row| self.row_to_symbol(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(symbols)
    }

    /// Query symbols by name pattern (LIKE match)
    pub fn query_symbol_by_pattern(&self, pattern: &str) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, kind, file_path, line_start, line_end, signature, parent_symbol, visibility, language
             FROM symbols WHERE name LIKE ?1 ORDER BY file_path LIMIT 100",
        )?;

        let like_pattern = format!("%{}%", pattern);
        let symbols = stmt
            .query_map(params![like_pattern], |row| self.row_to_symbol(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(symbols)
    }

    /// Query all symbols in a file
    pub fn query_symbols_in_file(&self, file_path: &Path) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, kind, file_path, line_start, line_end, signature, parent_symbol, visibility, language
             FROM symbols WHERE file_path = ?1 ORDER BY line_start",
        )?;

        let symbols = stmt
            .query_map(params![file_path.to_string_lossy()], |row| {
                self.row_to_symbol(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(symbols)
    }

    /// Query symbols by kind
    pub fn query_symbols_by_kind(&self, kind: SymbolKind, limit: usize) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, kind, file_path, line_start, line_end, signature, parent_symbol, visibility, language
             FROM symbols WHERE kind = ?1 ORDER BY name LIMIT ?2",
        )?;

        let symbols = stmt
            .query_map(params![kind.as_str(), limit], |row| self.row_to_symbol(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(symbols)
    }

    /// Delete all symbols for a file
    pub fn delete_file_symbols(&self, file_path: &Path) -> Result<usize> {
        let count = self.conn.execute(
            "DELETE FROM symbols WHERE file_path = ?1",
            params![file_path.to_string_lossy()],
        )?;
        Ok(count)
    }

    fn row_to_symbol(&self, row: &rusqlite::Row) -> rusqlite::Result<Symbol> {
        let kind_str: String = row.get(1)?;
        let file_path_str: String = row.get(2)?;
        let visibility_str: String = row.get(7)?;

        Ok(Symbol {
            name: row.get(0)?,
            kind: SymbolKind::from_str(&kind_str).unwrap_or(SymbolKind::Variable),
            file_path: PathBuf::from(file_path_str),
            line_start: row.get(3)?,
            line_end: row.get(4)?,
            signature: row.get(5)?,
            parent: row.get(6)?,
            visibility: Visibility::from_str(&visibility_str).unwrap_or(Visibility::Private),
            language: row.get(8)?,
        })
    }

    // ==================== Dependency Operations ====================

    /// Insert a dependency into the database
    pub fn insert_dependency(&self, dep: &Dependency) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();

        self.conn.execute(
            "INSERT INTO dependencies
             (source_file, target_file, kind, symbol_name, line_number, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                dep.source_file.to_string_lossy(),
                dep.target_file.as_ref().map(|p| p.to_string_lossy().to_string()),
                dep.kind.as_str(),
                dep.symbol_name,
                dep.line_number,
                now,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Query dependencies for a source file
    pub fn query_dependencies_from(&self, source_file: &Path) -> Result<Vec<Dependency>> {
        let mut stmt = self.conn.prepare(
            "SELECT source_file, target_file, kind, symbol_name, line_number
             FROM dependencies WHERE source_file = ?1 ORDER BY line_number",
        )?;

        let deps = stmt
            .query_map(params![source_file.to_string_lossy()], |row| {
                self.row_to_dependency(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(deps)
    }

    /// Query files that depend on a target file
    pub fn query_dependents_of(&self, target_file: &Path) -> Result<Vec<Dependency>> {
        let mut stmt = self.conn.prepare(
            "SELECT source_file, target_file, kind, symbol_name, line_number
             FROM dependencies WHERE target_file = ?1 ORDER BY source_file",
        )?;

        let deps = stmt
            .query_map(params![target_file.to_string_lossy()], |row| {
                self.row_to_dependency(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(deps)
    }

    /// Query all dependencies in the database
    pub fn query_all_dependencies(&self, limit: Option<usize>) -> Result<Vec<Dependency>> {
        let query = match limit {
            Some(lim) => format!(
                "SELECT source_file, target_file, kind, symbol_name, line_number
                 FROM dependencies ORDER BY source_file, line_number LIMIT {}",
                lim
            ),
            None => "SELECT source_file, target_file, kind, symbol_name, line_number
                     FROM dependencies ORDER BY source_file, line_number"
                .to_string(),
        };

        let mut stmt = self.conn.prepare(&query)?;

        let deps = stmt
            .query_map([], |row| self.row_to_dependency(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(deps)
    }

    /// Delete all dependencies for a source file
    pub fn delete_file_dependencies(&self, source_file: &Path) -> Result<usize> {
        let count = self.conn.execute(
            "DELETE FROM dependencies WHERE source_file = ?1",
            params![source_file.to_string_lossy()],
        )?;
        Ok(count)
    }

    fn row_to_dependency(&self, row: &rusqlite::Row) -> rusqlite::Result<Dependency> {
        let source_str: String = row.get(0)?;
        let target_str: Option<String> = row.get(1)?;
        let kind_str: String = row.get(2)?;

        Ok(Dependency {
            source_file: PathBuf::from(source_str),
            target_file: target_str.map(PathBuf::from),
            kind: DependencyKind::from_str(&kind_str).unwrap_or(DependencyKind::Import),
            symbol_name: row.get(3)?,
            line_number: row.get(4)?,
        })
    }

    // ==================== Call Operations ====================

    /// Insert a call relationship into the database
    pub fn insert_call(&self, call: &Call) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();

        self.conn.execute(
            "INSERT INTO calls
             (caller_symbol_id, callee_name, callee_symbol_id, call_file, call_line, call_column, is_method, is_async, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                call.caller_symbol_id,
                call.callee_name,
                call.callee_symbol_id,
                call.call_file.to_string_lossy(),
                call.call_line,
                call.call_column,
                call.is_method,
                call.is_async,
                now,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Query callers of a symbol by name
    pub fn query_callers(&self, callee_name: &str) -> Result<Vec<Call>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.caller_symbol_id, c.callee_name, c.callee_symbol_id, c.call_file, c.call_line, c.call_column, c.is_method, c.is_async, s.name as caller_name
             FROM calls c
             LEFT JOIN symbols s ON c.caller_symbol_id = s.id
             WHERE c.callee_name LIKE ?1
             ORDER BY c.call_file, c.call_line",
        )?;

        let pattern = format!("%{}", callee_name);
        let calls = stmt
            .query_map(params![pattern], |row| self.row_to_call(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(calls)
    }

    /// Query functions/methods called by a symbol
    pub fn query_callees(&self, caller_symbol_id: i64) -> Result<Vec<Call>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.caller_symbol_id, c.callee_name, c.callee_symbol_id, c.call_file, c.call_line, c.call_column, c.is_method, c.is_async, s.name as caller_name
             FROM calls c
             LEFT JOIN symbols s ON c.caller_symbol_id = s.id
             WHERE c.caller_symbol_id = ?1
             ORDER BY c.call_line",
        )?;

        let calls = stmt
            .query_map(params![caller_symbol_id], |row| self.row_to_call(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(calls)
    }

    /// Query callees by caller name
    pub fn query_callees_by_name(&self, caller_name: &str) -> Result<Vec<Call>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.caller_symbol_id, c.callee_name, c.callee_symbol_id, c.call_file, c.call_line, c.call_column, c.is_method, c.is_async, s.name as caller_name
             FROM calls c
             LEFT JOIN symbols s ON c.caller_symbol_id = s.id
             WHERE s.name = ?1
             ORDER BY c.call_line",
        )?;

        let calls = stmt
            .query_map(params![caller_name], |row| self.row_to_call(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(calls)
    }

    /// Query all calls in a file
    pub fn query_calls_in_file(&self, file_path: &Path) -> Result<Vec<Call>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.caller_symbol_id, c.callee_name, c.callee_symbol_id, c.call_file, c.call_line, c.call_column, c.is_method, c.is_async, s.name as caller_name
             FROM calls c
             LEFT JOIN symbols s ON c.caller_symbol_id = s.id
             WHERE c.call_file = ?1
             ORDER BY c.call_line",
        )?;

        let calls = stmt
            .query_map(params![file_path.to_string_lossy()], |row| {
                self.row_to_call(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(calls)
    }

    /// Delete all calls for a file
    pub fn delete_file_calls(&self, file_path: &Path) -> Result<usize> {
        let count = self.conn.execute(
            "DELETE FROM calls WHERE call_file = ?1",
            params![file_path.to_string_lossy()],
        )?;
        Ok(count)
    }

    fn row_to_call(&self, row: &rusqlite::Row) -> rusqlite::Result<Call> {
        let file_str: String = row.get(3)?;
        let caller_name: Option<String> = row.get(8)?;

        Ok(Call {
            caller_symbol_id: row.get(0)?,
            caller_name: caller_name.unwrap_or_default(),
            callee_name: row.get(1)?,
            callee_symbol_id: row.get(2)?,
            call_file: PathBuf::from(file_str),
            call_line: row.get(4)?,
            call_column: row.get(5)?,
            is_method: row.get(6)?,
            is_async: row.get(7)?,
        })
    }

    // ==================== Implementation Operations ====================

    /// Insert an implementation relationship
    pub fn insert_implementation(&self, impl_rel: &Implementation) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();

        self.conn.execute(
            "INSERT INTO implementations
             (implementor_id, trait_id, trait_name, impl_file, impl_line, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                impl_rel.implementor_id,
                impl_rel.trait_id,
                impl_rel.trait_name,
                impl_rel.impl_file.to_string_lossy(),
                impl_rel.impl_line,
                now,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Query all types that implement a trait
    pub fn query_implementors(&self, trait_name: &str) -> Result<Vec<Implementation>> {
        let mut stmt = self.conn.prepare(
            "SELECT i.implementor_id, i.trait_id, i.trait_name, i.impl_file, i.impl_line, s.name as implementor_name
             FROM implementations i
             LEFT JOIN symbols s ON i.implementor_id = s.id
             WHERE i.trait_name LIKE ?1
             ORDER BY i.impl_file, i.impl_line",
        )?;

        let pattern = format!("%{}", trait_name);
        let impls = stmt
            .query_map(params![pattern], |row| self.row_to_implementation(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(impls)
    }

    /// Query all traits implemented by a type
    pub fn query_traits_for(&self, implementor_name: &str) -> Result<Vec<Implementation>> {
        let mut stmt = self.conn.prepare(
            "SELECT i.implementor_id, i.trait_id, i.trait_name, i.impl_file, i.impl_line, s.name as implementor_name
             FROM implementations i
             LEFT JOIN symbols s ON i.implementor_id = s.id
             WHERE s.name = ?1
             ORDER BY i.trait_name",
        )?;

        let impls = stmt
            .query_map(params![implementor_name], |row| {
                self.row_to_implementation(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(impls)
    }

    /// Delete all implementations for a file
    pub fn delete_file_implementations(&self, file_path: &Path) -> Result<usize> {
        let count = self.conn.execute(
            "DELETE FROM implementations WHERE impl_file = ?1",
            params![file_path.to_string_lossy()],
        )?;
        Ok(count)
    }

    fn row_to_implementation(&self, row: &rusqlite::Row) -> rusqlite::Result<Implementation> {
        let file_str: String = row.get(3)?;
        let implementor_name: Option<String> = row.get(5)?;

        Ok(Implementation {
            implementor_id: row.get(0)?,
            implementor_name: implementor_name.unwrap_or_default(),
            trait_id: row.get(1)?,
            trait_name: row.get(2)?,
            impl_file: PathBuf::from(file_str),
            impl_line: row.get(4)?,
        })
    }

    // ==================== Usage Operations ====================

    /// Insert a symbol usage
    pub fn insert_usage(&self, usage: &Usage) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();

        self.conn.execute(
            "INSERT INTO usages
             (symbol_id, symbol_name, usage_kind, usage_file, usage_line, usage_column, context_symbol_id, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                usage.symbol_id,
                usage.symbol_name,
                usage.kind.as_str(),
                usage.usage_file.to_string_lossy(),
                usage.usage_line,
                usage.usage_column,
                usage.context_symbol_id,
                now,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Query all usages of a symbol by name
    pub fn query_usages(&self, symbol_name: &str) -> Result<Vec<Usage>> {
        let mut stmt = self.conn.prepare(
            "SELECT u.symbol_id, u.symbol_name, u.usage_kind, u.usage_file, u.usage_line, u.usage_column, u.context_symbol_id, s.name as context_name
             FROM usages u
             LEFT JOIN symbols s ON u.context_symbol_id = s.id
             WHERE u.symbol_name LIKE ?1
             ORDER BY u.usage_file, u.usage_line",
        )?;

        let pattern = format!("%{}", symbol_name);
        let usages = stmt
            .query_map(params![pattern], |row| self.row_to_usage(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(usages)
    }

    /// Query usages of a symbol filtered by kind
    pub fn query_usages_by_kind(&self, symbol_name: &str, kind: UsageKind) -> Result<Vec<Usage>> {
        let mut stmt = self.conn.prepare(
            "SELECT u.symbol_id, u.symbol_name, u.usage_kind, u.usage_file, u.usage_line, u.usage_column, u.context_symbol_id, s.name as context_name
             FROM usages u
             LEFT JOIN symbols s ON u.context_symbol_id = s.id
             WHERE u.symbol_name LIKE ?1 AND u.usage_kind = ?2
             ORDER BY u.usage_file, u.usage_line",
        )?;

        let pattern = format!("%{}", symbol_name);
        let usages = stmt
            .query_map(params![pattern, kind.as_str()], |row| {
                self.row_to_usage(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(usages)
    }

    /// Query usages in a specific file
    pub fn query_usages_in_file(&self, file_path: &Path) -> Result<Vec<Usage>> {
        let mut stmt = self.conn.prepare(
            "SELECT u.symbol_id, u.symbol_name, u.usage_kind, u.usage_file, u.usage_line, u.usage_column, u.context_symbol_id, s.name as context_name
             FROM usages u
             LEFT JOIN symbols s ON u.context_symbol_id = s.id
             WHERE u.usage_file = ?1
             ORDER BY u.usage_line",
        )?;

        let usages = stmt
            .query_map(params![file_path.to_string_lossy()], |row| {
                self.row_to_usage(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(usages)
    }

    /// Find potentially unused symbols (defined but not referenced)
    pub fn query_unused_symbols(&self, file_path: Option<&Path>) -> Result<Vec<Symbol>> {
        let query = match file_path {
            Some(_) => {
                "SELECT s.name, s.kind, s.file_path, s.line_start, s.line_end, s.signature, s.parent_symbol, s.visibility, s.language
                 FROM symbols s
                 LEFT JOIN usages u ON s.name = u.symbol_name
                 WHERE s.file_path = ?1 AND u.id IS NULL AND s.visibility != 'private'
                 ORDER BY s.file_path, s.line_start"
            }
            None => {
                "SELECT s.name, s.kind, s.file_path, s.line_start, s.line_end, s.signature, s.parent_symbol, s.visibility, s.language
                 FROM symbols s
                 LEFT JOIN usages u ON s.name = u.symbol_name
                 WHERE u.id IS NULL AND s.visibility != 'private'
                 ORDER BY s.file_path, s.line_start"
            }
        };

        let mut stmt = self.conn.prepare(query)?;

        let symbols = match file_path {
            Some(path) => stmt
                .query_map(params![path.to_string_lossy()], |row| self.row_to_symbol(row))?
                .collect::<std::result::Result<Vec<_>, _>>()?,
            None => stmt
                .query_map([], |row| self.row_to_symbol(row))?
                .collect::<std::result::Result<Vec<_>, _>>()?,
        };

        Ok(symbols)
    }

    /// Delete all usages for a file
    pub fn delete_file_usages(&self, file_path: &Path) -> Result<usize> {
        let count = self.conn.execute(
            "DELETE FROM usages WHERE usage_file = ?1",
            params![file_path.to_string_lossy()],
        )?;
        Ok(count)
    }

    fn row_to_usage(&self, row: &rusqlite::Row) -> rusqlite::Result<Usage> {
        let file_str: String = row.get(3)?;
        let kind_str: String = row.get(2)?;
        let context_name: Option<String> = row.get(7)?;

        Ok(Usage {
            symbol_id: row.get(0)?,
            symbol_name: row.get(1)?,
            kind: UsageKind::from_str(&kind_str).unwrap_or(UsageKind::Variable),
            usage_file: PathBuf::from(file_str),
            usage_line: row.get(4)?,
            usage_column: row.get(5)?,
            context_symbol_id: row.get(6)?,
            context_name,
        })
    }

    // ==================== File Metadata Operations ====================

    /// Insert or update file metadata
    pub fn upsert_file(&self, file: &FileMetadata) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        self.conn.execute(
            "INSERT OR REPLACE INTO files
             (path, size, last_modified, change_count, hotness_score, language, lines_of_code, indexed_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                file.path.to_string_lossy(),
                file.size,
                file.last_modified,
                file.change_count,
                file.hotness_score,
                file.language,
                file.lines_of_code,
                now,
                file.status.as_str(),
            ],
        )?;

        Ok(())
    }

    /// Get file metadata by path
    pub fn get_file(&self, path: &Path) -> Result<Option<FileMetadata>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, size, last_modified, change_count, hotness_score, language, lines_of_code, status
             FROM files WHERE path = ?1",
        )?;

        let file = stmt
            .query_row(params![path.to_string_lossy()], |row| {
                self.row_to_file_metadata(row)
            })
            .optional()?;

        Ok(file)
    }

    /// Get hot files (sorted by hotness score descending)
    pub fn get_hot_files(&self, limit: usize) -> Result<Vec<FileMetadata>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, size, last_modified, change_count, hotness_score, language, lines_of_code, status
             FROM files WHERE status = 'indexed' ORDER BY hotness_score DESC LIMIT ?1",
        )?;

        let files = stmt
            .query_map(params![limit], |row| self.row_to_file_metadata(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(files)
    }

    /// Mark a file as deleted
    pub fn mark_file_deleted(&self, path: &Path) -> Result<()> {
        self.conn.execute(
            "UPDATE files SET status = 'deleted' WHERE path = ?1",
            params![path.to_string_lossy()],
        )?;
        Ok(())
    }

    /// Delete all data for a file (symbols, deps, calls, implementations, usages, metadata)
    pub fn delete_file_data(&self, path: &Path) -> Result<()> {
        self.delete_file_calls(path)?;
        self.delete_file_implementations(path)?;
        self.delete_file_usages(path)?;
        self.delete_file_symbols(path)?;
        self.delete_file_dependencies(path)?;
        // Don't delete file metadata, just update it
        Ok(())
    }

    fn row_to_file_metadata(&self, row: &rusqlite::Row) -> rusqlite::Result<FileMetadata> {
        let path_str: String = row.get(0)?;
        let status_str: String = row.get(7)?;

        Ok(FileMetadata {
            path: PathBuf::from(path_str),
            size: row.get(1)?,
            last_modified: row.get(2)?,
            change_count: row.get(3)?,
            hotness_score: row.get(4)?,
            language: row.get(5)?,
            lines_of_code: row.get(6)?,
            status: FileStatus::from_str(&status_str).unwrap_or(FileStatus::Indexed),
        })
    }

    // ==================== Statistics ====================

    /// Get index statistics
    pub fn get_stats(&self) -> Result<IndexStats> {
        let total_files: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM files WHERE status = 'indexed'", [], |r| {
                r.get(0)
            })?;

        let total_symbols: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))?;

        let total_dependencies: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM dependencies", [], |r| r.get(0))?;

        let total_calls: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM calls", [], |r| r.get(0))?;

        let total_implementations: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM implementations", [], |r| r.get(0))?;

        let total_usages: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM usages", [], |r| r.get(0))?;

        // Get language breakdown
        let mut stmt = self.conn.prepare(
            "SELECT language, COUNT(*) as count FROM files WHERE status = 'indexed' AND language IS NOT NULL
             GROUP BY language ORDER BY count DESC",
        )?;
        let languages: Vec<(String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Get database size
        let db_size: i64 = self
            .conn
            .query_row("SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);

        // Get last indexed time
        let last_indexed: Option<i64> = self
            .conn
            .query_row("SELECT MAX(indexed_at) FROM files", [], |r| r.get(0))
            .optional()?
            .flatten();

        Ok(IndexStats {
            total_files: total_files as usize,
            total_symbols: total_symbols as usize,
            total_dependencies: total_dependencies as usize,
            total_calls: total_calls as usize,
            total_implementations: total_implementations as usize,
            total_usages: total_usages as usize,
            languages,
            database_size_bytes: db_size as u64,
            last_indexed,
        })
    }

    // ==================== Maintenance ====================

    /// Clear all data from the database
    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute("DELETE FROM calls", [])?;
        self.conn.execute("DELETE FROM implementations", [])?;
        self.conn.execute("DELETE FROM usages", [])?;
        self.conn.execute("DELETE FROM symbols", [])?;
        self.conn.execute("DELETE FROM dependencies", [])?;
        self.conn.execute("DELETE FROM files", [])?;
        self.conn.execute("DELETE FROM project_config", [])?;
        info!("Database cleared");
        Ok(())
    }

    /// Get/set project configuration
    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM project_config WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?;
        Ok(value)
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO project_config (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }
}

/// Index statistics
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub total_files: usize,
    pub total_symbols: usize,
    pub total_dependencies: usize,
    pub total_calls: usize,
    pub total_implementations: usize,
    pub total_usages: usize,
    pub languages: Vec<(String, i64)>,
    pub database_size_bytes: u64,
    pub last_indexed: Option<i64>,
}

impl IndexStats {
    pub fn format_size(&self) -> String {
        let bytes = self.database_size_bytes;
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        }
    }

    pub fn format_last_indexed(&self) -> String {
        match self.last_indexed {
            Some(ts) => {
                let now = chrono::Utc::now().timestamp();
                let diff = now - ts;
                if diff < 60 {
                    format!("{} seconds ago", diff)
                } else if diff < 3600 {
                    format!("{} minutes ago", diff / 60)
                } else if diff < 86400 {
                    format!("{} hours ago", diff / 3600)
                } else {
                    format!("{} days ago", diff / 86400)
                }
            }
            None => "never".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_creation() {
        let db = Database::in_memory().unwrap();
        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_symbols, 0);
    }

    #[test]
    fn test_symbol_crud() {
        let db = Database::in_memory().unwrap();

        let symbol = Symbol::new("test_fn", SymbolKind::Function, "/test.rs", 10, 20, "rust")
            .with_signature("fn test_fn() -> i32")
            .with_visibility(Visibility::Public);

        // Insert
        let id = db.insert_symbol(&symbol).unwrap();
        assert!(id > 0);

        // Query by name
        let results = db.query_symbol_by_name("test_fn").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "test_fn");
        assert_eq!(results[0].kind, SymbolKind::Function);
        assert_eq!(results[0].signature, Some("fn test_fn() -> i32".to_string()));

        // Query by pattern
        let results = db.query_symbol_by_pattern("test").unwrap();
        assert_eq!(results.len(), 1);

        // Query by file
        let results = db.query_symbols_in_file(Path::new("/test.rs")).unwrap();
        assert_eq!(results.len(), 1);

        // Delete
        let count = db.delete_file_symbols(Path::new("/test.rs")).unwrap();
        assert_eq!(count, 1);

        let results = db.query_symbol_by_name("test_fn").unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_dependency_crud() {
        let db = Database::in_memory().unwrap();

        let dep = Dependency::new("/src/main.rs", DependencyKind::Import, 5)
            .with_target("/src/lib.rs")
            .with_symbol("Config");

        // Insert
        let id = db.insert_dependency(&dep).unwrap();
        assert!(id > 0);

        // Query from source
        let results = db
            .query_dependencies_from(Path::new("/src/main.rs"))
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].symbol_name, Some("Config".to_string()));

        // Query dependents
        let results = db
            .query_dependents_of(Path::new("/src/lib.rs"))
            .unwrap();
        assert_eq!(results.len(), 1);

        // Delete
        let count = db
            .delete_file_dependencies(Path::new("/src/main.rs"))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_file_metadata() {
        let db = Database::in_memory().unwrap();

        let mut file = FileMetadata::new("/test.rs", 1024, 12345)
            .with_language("rust")
            .with_lines_of_code(50);
        file.calculate_hotness();

        // Insert
        db.upsert_file(&file).unwrap();

        // Get
        let result = db.get_file(Path::new("/test.rs")).unwrap().unwrap();
        assert_eq!(result.size, 1024);
        assert_eq!(result.language, Some("rust".to_string()));
        assert_eq!(result.lines_of_code, 50);

        // Update (increment change_count)
        file.change_count = 5;
        file.calculate_hotness();
        db.upsert_file(&file).unwrap();

        let result = db.get_file(Path::new("/test.rs")).unwrap().unwrap();
        assert_eq!(result.change_count, 5);

        // Hot files
        let hot = db.get_hot_files(10).unwrap();
        assert_eq!(hot.len(), 1);
    }

    #[test]
    fn test_hot_files_ordering() {
        let db = Database::in_memory().unwrap();

        // Create files with different hotness
        for i in 1..=5 {
            let mut file = FileMetadata::new(format!("/test{}.rs", i), 100, 0)
                .with_language("rust")
                .with_lines_of_code(100);
            file.change_count = i as u32;
            file.calculate_hotness();
            db.upsert_file(&file).unwrap();
        }

        let hot = db.get_hot_files(3).unwrap();
        assert_eq!(hot.len(), 3);
        // Should be sorted by hotness descending
        assert!(hot[0].hotness_score >= hot[1].hotness_score);
        assert!(hot[1].hotness_score >= hot[2].hotness_score);
    }

    #[test]
    fn test_stats() {
        let db = Database::in_memory().unwrap();

        // Add some data
        db.insert_symbol(&Symbol::new(
            "fn1",
            SymbolKind::Function,
            "/test.rs",
            1,
            10,
            "rust",
        ))
        .unwrap();
        db.insert_symbol(&Symbol::new(
            "fn2",
            SymbolKind::Function,
            "/test.ts",
            1,
            10,
            "typescript",
        ))
        .unwrap();

        db.upsert_file(&FileMetadata::new("/test.rs", 100, 0).with_language("rust"))
            .unwrap();
        db.upsert_file(&FileMetadata::new("/test.ts", 100, 0).with_language("typescript"))
            .unwrap();

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_files, 2);
        assert_eq!(stats.total_symbols, 2);
        assert_eq!(stats.languages.len(), 2);
    }

    #[test]
    fn test_clear_all() {
        let db = Database::in_memory().unwrap();

        db.insert_symbol(&Symbol::new(
            "fn1",
            SymbolKind::Function,
            "/test.rs",
            1,
            10,
            "rust",
        ))
        .unwrap();
        db.upsert_file(&FileMetadata::new("/test.rs", 100, 0))
            .unwrap();

        db.clear_all().unwrap();

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_symbols, 0);
    }

    #[test]
    fn test_config() {
        let db = Database::in_memory().unwrap();

        db.set_config("test_key", "test_value").unwrap();
        let value = db.get_config("test_key").unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        let missing = db.get_config("nonexistent").unwrap();
        assert_eq!(missing, None);
    }
}
