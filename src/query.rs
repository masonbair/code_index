//! Query API for the code index

use crate::error::Result;
use crate::indexer::{Call, Database, Dependency, FileMetadata, Implementation, Symbol, SymbolKind, Usage, UsageKind};
use serde::Serialize;
use std::path::Path;

/// Query interface for the code index
pub struct QueryEngine<'a> {
    db: &'a Database,
}

impl<'a> QueryEngine<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Find symbols by exact name
    pub fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
        self.db.query_symbol_by_name(name)
    }

    /// Find symbols by pattern (partial match)
    pub fn search_symbols(&self, pattern: &str) -> Result<Vec<Symbol>> {
        self.db.query_symbol_by_pattern(pattern)
    }

    /// Get all symbols in a file
    pub fn symbols_in_file(&self, file_path: &Path) -> Result<Vec<Symbol>> {
        self.db.query_symbols_in_file(file_path)
    }

    /// Get symbols by kind
    pub fn symbols_by_kind(&self, kind: SymbolKind, limit: usize) -> Result<Vec<Symbol>> {
        self.db.query_symbols_by_kind(kind, limit)
    }

    /// Get dependencies from a file
    pub fn dependencies_from(&self, file_path: &Path) -> Result<Vec<Dependency>> {
        self.db.query_dependencies_from(file_path)
    }

    /// Get files that depend on a given file
    pub fn dependents_of(&self, file_path: &Path) -> Result<Vec<Dependency>> {
        self.db.query_dependents_of(file_path)
    }

    /// Get all dependencies in the index
    pub fn all_dependencies(&self, limit: Option<usize>) -> Result<Vec<Dependency>> {
        self.db.query_all_dependencies(limit)
    }

    /// Get file metadata
    pub fn file_info(&self, file_path: &Path) -> Result<Option<FileMetadata>> {
        self.db.get_file(file_path)
    }

    /// Get hot files (frequently changed / complex)
    pub fn hot_files(&self, limit: usize) -> Result<Vec<FileMetadata>> {
        self.db.get_hot_files(limit)
    }

    // ==================== Call Graph Queries ====================

    /// Find all callers of a symbol
    pub fn callers(&self, callee_name: &str) -> Result<Vec<Call>> {
        self.db.query_callers(callee_name)
    }

    /// Find all functions/methods called by a symbol
    pub fn callees(&self, caller_name: &str) -> Result<Vec<Call>> {
        self.db.query_callees_by_name(caller_name)
    }

    /// Get all calls in a file
    pub fn calls_in_file(&self, file_path: &Path) -> Result<Vec<Call>> {
        self.db.query_calls_in_file(file_path)
    }

    // ==================== Implementation Queries ====================

    /// Find all types that implement a trait
    pub fn implementors(&self, trait_name: &str) -> Result<Vec<Implementation>> {
        self.db.query_implementors(trait_name)
    }

    /// Find all traits implemented by a type
    pub fn traits_for(&self, type_name: &str) -> Result<Vec<Implementation>> {
        self.db.query_traits_for(type_name)
    }

    // ==================== Usage Queries ====================

    /// Find all usages of a symbol
    pub fn usages(&self, symbol_name: &str) -> Result<Vec<Usage>> {
        self.db.query_usages(symbol_name)
    }

    /// Find usages filtered by kind
    pub fn usages_by_kind(&self, symbol_name: &str, kind: UsageKind) -> Result<Vec<Usage>> {
        self.db.query_usages_by_kind(symbol_name, kind)
    }

    /// Find usages in a specific file
    pub fn usages_in_file(&self, file_path: &Path) -> Result<Vec<Usage>> {
        self.db.query_usages_in_file(file_path)
    }

    /// Find potentially unused symbols
    pub fn unused_symbols(&self, file_path: Option<&Path>) -> Result<Vec<Symbol>> {
        self.db.query_unused_symbols(file_path)
    }
}

/// JSON output structures for CLI

#[derive(Serialize)]
pub struct SymbolResult {
    pub file: String,
    pub line: usize,
    pub line_end: usize,
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub visibility: String,
    pub language: String,
}

impl From<Symbol> for SymbolResult {
    fn from(s: Symbol) -> Self {
        Self {
            file: s.file_path.to_string_lossy().to_string(),
            line: s.line_start,
            line_end: s.line_end,
            kind: s.kind.as_str().to_string(),
            name: s.name,
            signature: s.signature,
            parent: s.parent,
            visibility: s.visibility.as_str().to_string(),
            language: s.language,
        }
    }
}

#[derive(Serialize)]
pub struct SymbolQueryResult {
    pub query: String,
    pub count: usize,
    pub results: Vec<SymbolResult>,
}

#[derive(Serialize)]
pub struct DependencyResult {
    pub source_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_file: Option<String>,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub line: usize,
}

impl From<Dependency> for DependencyResult {
    fn from(d: Dependency) -> Self {
        Self {
            source_file: d.source_file.to_string_lossy().to_string(),
            target_file: d.target_file.map(|p| p.to_string_lossy().to_string()),
            kind: d.kind.as_str().to_string(),
            symbol: d.symbol_name,
            line: d.line_number,
        }
    }
}

#[derive(Serialize)]
pub struct DependencyQueryResult {
    pub file: String,
    pub direction: String, // "from" or "to"
    pub count: usize,
    pub dependencies: Vec<DependencyResult>,
}

#[derive(Serialize)]
pub struct FileResult {
    pub path: String,
    pub size: u64,
    pub lines_of_code: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub change_count: u32,
    pub hotness_score: f64,
    pub status: String,
}

impl From<FileMetadata> for FileResult {
    fn from(f: FileMetadata) -> Self {
        Self {
            path: f.path.to_string_lossy().to_string(),
            size: f.size,
            lines_of_code: f.lines_of_code,
            language: f.language,
            change_count: f.change_count,
            hotness_score: f.hotness_score,
            status: f.status.as_str().to_string(),
        }
    }
}

#[derive(Serialize)]
pub struct HotFilesResult {
    pub limit: usize,
    pub count: usize,
    pub files: Vec<FileResult>,
}

#[derive(Serialize)]
pub struct StatsResult {
    pub total_files: usize,
    pub total_symbols: usize,
    pub total_dependencies: usize,
    pub total_calls: usize,
    pub total_implementations: usize,
    pub total_usages: usize,
    pub languages: Vec<LanguageStat>,
    pub database_size: String,
    pub last_indexed: String,
}

#[derive(Serialize)]
pub struct LanguageStat {
    pub language: String,
    pub file_count: i64,
    pub percentage: f64,
}

// ==================== Call Results ====================

#[derive(Serialize)]
pub struct CallResult {
    pub caller: String,
    pub callee: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub is_method: bool,
    pub is_async: bool,
}

impl From<Call> for CallResult {
    fn from(c: Call) -> Self {
        Self {
            caller: c.caller_name,
            callee: c.callee_name,
            file: c.call_file.to_string_lossy().to_string(),
            line: c.call_line,
            column: c.call_column,
            is_method: c.is_method,
            is_async: c.is_async,
        }
    }
}

#[derive(Serialize)]
pub struct CallersResult {
    pub callee: String,
    pub count: usize,
    pub callers: Vec<CallResult>,
}

#[derive(Serialize)]
pub struct CalleesResult {
    pub caller: String,
    pub count: usize,
    pub callees: Vec<CallResult>,
}

// ==================== Implementation Results ====================

#[derive(Serialize)]
pub struct ImplementationResult {
    pub implementor: String,
    pub trait_name: String,
    pub file: String,
    pub line: usize,
}

impl From<Implementation> for ImplementationResult {
    fn from(i: Implementation) -> Self {
        Self {
            implementor: i.implementor_name,
            trait_name: i.trait_name,
            file: i.impl_file.to_string_lossy().to_string(),
            line: i.impl_line,
        }
    }
}

#[derive(Serialize)]
pub struct ImplementorsResult {
    pub trait_name: String,
    pub count: usize,
    pub implementors: Vec<ImplementationResult>,
}

#[derive(Serialize)]
pub struct TraitsResult {
    pub type_name: String,
    pub count: usize,
    pub traits: Vec<ImplementationResult>,
}

// ==================== Usage Results ====================

#[derive(Serialize)]
pub struct UsageResult {
    pub symbol: String,
    pub kind: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

impl From<Usage> for UsageResult {
    fn from(u: Usage) -> Self {
        Self {
            symbol: u.symbol_name,
            kind: u.kind.as_str().to_string(),
            file: u.usage_file.to_string_lossy().to_string(),
            line: u.usage_line,
            column: u.usage_column,
            context: u.context_name,
        }
    }
}

#[derive(Serialize)]
pub struct UsagesQueryResult {
    pub symbol: String,
    pub count: usize,
    pub usages: Vec<UsageResult>,
}

#[derive(Serialize)]
pub struct UnusedResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub count: usize,
    pub symbols: Vec<SymbolResult>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::{DependencyKind, Visibility};

    #[test]
    fn test_symbol_result_conversion() {
        let symbol = Symbol::new("test", SymbolKind::Function, "/test.rs", 1, 10, "rust")
            .with_signature("fn test()")
            .with_visibility(Visibility::Public);

        let result: SymbolResult = symbol.into();
        assert_eq!(result.name, "test");
        assert_eq!(result.kind, "function");
        assert_eq!(result.visibility, "public");
    }

    #[test]
    fn test_dependency_result_conversion() {
        let dep = Dependency::new("/src/main.rs", DependencyKind::Import, 5)
            .with_target("/src/lib.rs")
            .with_symbol("Config");

        let result: DependencyResult = dep.into();
        assert_eq!(result.source_file, "/src/main.rs");
        assert_eq!(result.target_file, Some("/src/lib.rs".to_string()));
        assert_eq!(result.symbol, Some("Config".to_string()));
    }

    #[test]
    fn test_query_engine() {
        let db = Database::in_memory().unwrap();
        let engine = QueryEngine::new(&db);

        // Empty database should return empty results
        let symbols = engine.find_symbol("nonexistent").unwrap();
        assert!(symbols.is_empty());

        let hot = engine.hot_files(10).unwrap();
        assert!(hot.is_empty());
    }
}
