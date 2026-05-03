//! Core data structures for symbols, dependencies, and file metadata

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Kind of symbol (function, class, struct, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Function,
    Class,
    Struct,
    Interface,
    Type,
    Variable,
    Constant,
    Enum,
    Module,
    Trait,
    Impl,
    Method,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Class => "class",
            SymbolKind::Struct => "struct",
            SymbolKind::Interface => "interface",
            SymbolKind::Type => "type",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
            SymbolKind::Enum => "enum",
            SymbolKind::Module => "module",
            SymbolKind::Trait => "trait",
            SymbolKind::Impl => "impl",
            SymbolKind::Method => "method",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "function" => Some(SymbolKind::Function),
            "class" => Some(SymbolKind::Class),
            "struct" => Some(SymbolKind::Struct),
            "interface" => Some(SymbolKind::Interface),
            "type" => Some(SymbolKind::Type),
            "variable" => Some(SymbolKind::Variable),
            "constant" => Some(SymbolKind::Constant),
            "enum" => Some(SymbolKind::Enum),
            "module" => Some(SymbolKind::Module),
            "trait" => Some(SymbolKind::Trait),
            "impl" => Some(SymbolKind::Impl),
            "method" => Some(SymbolKind::Method),
            _ => None,
        }
    }
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Visibility of a symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    #[default]
    Public,
    Private,
    Protected,
    Internal,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
            Visibility::Protected => "protected",
            Visibility::Internal => "internal",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "public" => Some(Visibility::Public),
            "private" => Some(Visibility::Private),
            "protected" => Some(Visibility::Protected),
            "internal" => Some(Visibility::Internal),
            _ => None,
        }
    }
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A symbol extracted from source code (function, class, struct, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// Name of the symbol
    pub name: String,
    /// Kind of symbol
    pub kind: SymbolKind,
    /// File path where the symbol is defined
    pub file_path: PathBuf,
    /// Starting line number (1-indexed)
    pub line_start: usize,
    /// Ending line number (1-indexed)
    pub line_end: usize,
    /// Full signature (for functions/methods)
    pub signature: Option<String>,
    /// Parent symbol (for methods inside classes, nested functions)
    pub parent: Option<String>,
    /// Visibility modifier
    pub visibility: Visibility,
    /// Programming language
    pub language: String,
}

impl Symbol {
    pub fn new(
        name: impl Into<String>,
        kind: SymbolKind,
        file_path: impl Into<PathBuf>,
        line_start: usize,
        line_end: usize,
        language: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            file_path: file_path.into(),
            line_start,
            line_end,
            signature: None,
            parent: None,
            visibility: Visibility::Public,
            language: language.into(),
        }
    }

    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }
}

/// Kind of dependency relationship
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Import,
    Call,
    Inheritance,
    TypeReference,
}

impl DependencyKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DependencyKind::Import => "import",
            DependencyKind::Call => "call",
            DependencyKind::Inheritance => "inheritance",
            DependencyKind::TypeReference => "type_reference",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "import" => Some(DependencyKind::Import),
            "call" => Some(DependencyKind::Call),
            "inheritance" => Some(DependencyKind::Inheritance),
            "type_reference" => Some(DependencyKind::TypeReference),
            _ => None,
        }
    }
}

impl std::fmt::Display for DependencyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A dependency relationship between files/symbols
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Source file that has the dependency
    pub source_file: PathBuf,
    /// Target file (None for external dependencies)
    pub target_file: Option<PathBuf>,
    /// Kind of dependency
    pub kind: DependencyKind,
    /// Symbol name being imported/called
    pub symbol_name: Option<String>,
    /// Line number where the dependency appears
    pub line_number: usize,
}

impl Dependency {
    pub fn new(
        source_file: impl Into<PathBuf>,
        kind: DependencyKind,
        line_number: usize,
    ) -> Self {
        Self {
            source_file: source_file.into(),
            target_file: None,
            kind,
            symbol_name: None,
            line_number,
        }
    }

    pub fn with_target(mut self, target: impl Into<PathBuf>) -> Self {
        self.target_file = Some(target.into());
        self
    }

    pub fn with_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol_name = Some(symbol.into());
        self
    }
}

/// A function/method call relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Call {
    /// Database ID of the calling symbol (optional, set after insert)
    pub caller_symbol_id: Option<i64>,
    /// Name of the caller function/method
    pub caller_name: String,
    /// Name of the called function/method
    pub callee_name: String,
    /// Database ID of the callee symbol (None if external)
    pub callee_symbol_id: Option<i64>,
    /// File where the call occurs
    pub call_file: PathBuf,
    /// Line number of the call
    pub call_line: usize,
    /// Column number of the call
    pub call_column: usize,
    /// Whether this is a method call (obj.method())
    pub is_method: bool,
    /// Whether this is an async call (.await)
    pub is_async: bool,
}

impl Call {
    pub fn new(
        caller_name: impl Into<String>,
        callee_name: impl Into<String>,
        call_file: impl Into<PathBuf>,
        call_line: usize,
        call_column: usize,
    ) -> Self {
        Self {
            caller_symbol_id: None,
            caller_name: caller_name.into(),
            callee_name: callee_name.into(),
            callee_symbol_id: None,
            call_file: call_file.into(),
            call_line,
            call_column,
            is_method: false,
            is_async: false,
        }
    }

    pub fn with_method(mut self, is_method: bool) -> Self {
        self.is_method = is_method;
        self
    }

    pub fn with_async(mut self, is_async: bool) -> Self {
        self.is_async = is_async;
        self
    }

    pub fn with_caller_id(mut self, id: i64) -> Self {
        self.caller_symbol_id = Some(id);
        self
    }

    pub fn with_callee_id(mut self, id: i64) -> Self {
        self.callee_symbol_id = Some(id);
        self
    }
}

/// A trait/interface implementation relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Implementation {
    /// Database ID of the implementor (struct/enum)
    pub implementor_id: Option<i64>,
    /// Name of the implementor type
    pub implementor_name: String,
    /// Database ID of the trait (None if external)
    pub trait_id: Option<i64>,
    /// Name of the trait being implemented
    pub trait_name: String,
    /// File where the impl block is
    pub impl_file: PathBuf,
    /// Line number of the impl block
    pub impl_line: usize,
}

impl Implementation {
    pub fn new(
        implementor_name: impl Into<String>,
        trait_name: impl Into<String>,
        impl_file: impl Into<PathBuf>,
        impl_line: usize,
    ) -> Self {
        Self {
            implementor_id: None,
            implementor_name: implementor_name.into(),
            trait_id: None,
            trait_name: trait_name.into(),
            impl_file: impl_file.into(),
            impl_line,
        }
    }

    pub fn with_implementor_id(mut self, id: i64) -> Self {
        self.implementor_id = Some(id);
        self
    }

    pub fn with_trait_id(mut self, id: i64) -> Self {
        self.trait_id = Some(id);
        self
    }
}

/// Kind of symbol usage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageKind {
    /// Type annotation (e.g., `x: Foo`)
    TypeAnnotation,
    /// Function/method call
    Call,
    /// Field access (e.g., `x.field`)
    FieldAccess,
    /// Variable reference
    Variable,
    /// Import statement
    Import,
    /// Return type
    ReturnType,
    /// Generic parameter
    GenericParam,
    /// Trait bound
    TraitBound,
}

impl UsageKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            UsageKind::TypeAnnotation => "type_annotation",
            UsageKind::Call => "call",
            UsageKind::FieldAccess => "field_access",
            UsageKind::Variable => "variable",
            UsageKind::Import => "import",
            UsageKind::ReturnType => "return_type",
            UsageKind::GenericParam => "generic_param",
            UsageKind::TraitBound => "trait_bound",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "type_annotation" => Some(UsageKind::TypeAnnotation),
            "call" => Some(UsageKind::Call),
            "field_access" => Some(UsageKind::FieldAccess),
            "variable" => Some(UsageKind::Variable),
            "import" => Some(UsageKind::Import),
            "return_type" => Some(UsageKind::ReturnType),
            "generic_param" => Some(UsageKind::GenericParam),
            "trait_bound" => Some(UsageKind::TraitBound),
            _ => None,
        }
    }
}

impl std::fmt::Display for UsageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A symbol usage/reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// Database ID of the referenced symbol (None if external)
    pub symbol_id: Option<i64>,
    /// Name of the referenced symbol
    pub symbol_name: String,
    /// Kind of usage
    pub kind: UsageKind,
    /// File where the usage occurs
    pub usage_file: PathBuf,
    /// Line number of the usage
    pub usage_line: usize,
    /// Column number of the usage
    pub usage_column: usize,
    /// Database ID of the enclosing function/method (context)
    pub context_symbol_id: Option<i64>,
    /// Name of the enclosing function/method
    pub context_name: Option<String>,
}

impl Usage {
    pub fn new(
        symbol_name: impl Into<String>,
        kind: UsageKind,
        usage_file: impl Into<PathBuf>,
        usage_line: usize,
        usage_column: usize,
    ) -> Self {
        Self {
            symbol_id: None,
            symbol_name: symbol_name.into(),
            kind,
            usage_file: usage_file.into(),
            usage_line,
            usage_column,
            context_symbol_id: None,
            context_name: None,
        }
    }

    pub fn with_symbol_id(mut self, id: i64) -> Self {
        self.symbol_id = Some(id);
        self
    }

    pub fn with_context(mut self, context_name: impl Into<String>) -> Self {
        self.context_name = Some(context_name.into());
        self
    }

    pub fn with_context_id(mut self, id: i64) -> Self {
        self.context_symbol_id = Some(id);
        self
    }
}

/// File status in the index
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    #[default]
    Indexed,
    Deleted,
    Error,
}

impl FileStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileStatus::Indexed => "indexed",
            FileStatus::Deleted => "deleted",
            FileStatus::Error => "error",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "indexed" => Some(FileStatus::Indexed),
            "deleted" => Some(FileStatus::Deleted),
            "error" => Some(FileStatus::Error),
            _ => None,
        }
    }
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Metadata about an indexed file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// File path
    pub path: PathBuf,
    /// File size in bytes
    pub size: u64,
    /// Last modified timestamp (Unix epoch)
    pub last_modified: i64,
    /// Number of times this file has been re-indexed
    pub change_count: u32,
    /// Hotness score (change_count * complexity_factor)
    pub hotness_score: f64,
    /// Programming language
    pub language: Option<String>,
    /// Lines of code
    pub lines_of_code: usize,
    /// File status
    pub status: FileStatus,
}

impl FileMetadata {
    pub fn new(path: impl Into<PathBuf>, size: u64, last_modified: i64) -> Self {
        Self {
            path: path.into(),
            size,
            last_modified,
            change_count: 1,
            hotness_score: 0.0,
            language: None,
            lines_of_code: 0,
            status: FileStatus::Indexed,
        }
    }

    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    pub fn with_lines_of_code(mut self, lines: usize) -> Self {
        self.lines_of_code = lines;
        self
    }

    /// Calculate hotness score based on change count and lines of code
    pub fn calculate_hotness(&mut self) {
        // Simple formula: more changes + larger files = higher hotness
        // Normalize lines_of_code to a factor (log scale to prevent huge files dominating)
        let size_factor = (self.lines_of_code as f64 + 1.0).ln();
        self.hotness_score = self.change_count as f64 * (1.0 + size_factor * 0.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_kind_roundtrip() {
        let kinds = [
            SymbolKind::Function,
            SymbolKind::Class,
            SymbolKind::Struct,
            SymbolKind::Interface,
            SymbolKind::Type,
            SymbolKind::Variable,
            SymbolKind::Constant,
            SymbolKind::Enum,
            SymbolKind::Module,
            SymbolKind::Trait,
            SymbolKind::Impl,
            SymbolKind::Method,
        ];

        for kind in kinds {
            let s = kind.as_str();
            let parsed = SymbolKind::from_str(s).unwrap();
            assert_eq!(kind, parsed);
        }
    }

    #[test]
    fn test_symbol_builder() {
        let symbol = Symbol::new("foo", SymbolKind::Function, "/test.rs", 10, 20, "rust")
            .with_signature("fn foo() -> i32")
            .with_parent("MyStruct")
            .with_visibility(Visibility::Private);

        assert_eq!(symbol.name, "foo");
        assert_eq!(symbol.kind, SymbolKind::Function);
        assert_eq!(symbol.signature, Some("fn foo() -> i32".to_string()));
        assert_eq!(symbol.parent, Some("MyStruct".to_string()));
        assert_eq!(symbol.visibility, Visibility::Private);
    }

    #[test]
    fn test_dependency_builder() {
        let dep = Dependency::new("/src/main.rs", DependencyKind::Import, 5)
            .with_target("/src/lib.rs")
            .with_symbol("Config");

        assert_eq!(dep.source_file, PathBuf::from("/src/main.rs"));
        assert_eq!(dep.target_file, Some(PathBuf::from("/src/lib.rs")));
        assert_eq!(dep.symbol_name, Some("Config".to_string()));
        assert_eq!(dep.kind, DependencyKind::Import);
    }

    #[test]
    fn test_file_metadata_hotness() {
        let mut meta = FileMetadata::new("/test.rs", 1000, 0).with_lines_of_code(100);
        meta.change_count = 5;
        meta.calculate_hotness();

        assert!(meta.hotness_score > 0.0);

        // More changes = higher hotness
        let mut meta2 = FileMetadata::new("/test2.rs", 1000, 0).with_lines_of_code(100);
        meta2.change_count = 10;
        meta2.calculate_hotness();

        assert!(meta2.hotness_score > meta.hotness_score);
    }
}
