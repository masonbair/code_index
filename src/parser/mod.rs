//! Language parsers using tree-sitter

pub mod python;
pub mod rust;
pub mod typescript;

use crate::error::Result;
use crate::indexer::{Call, Dependency, Implementation, Symbol, Usage};
use std::path::Path;

/// Result of parsing a file including all extracted relationships
#[derive(Debug, Default)]
pub struct ParseResult {
    pub symbols: Vec<Symbol>,
    pub dependencies: Vec<Dependency>,
    pub calls: Vec<Call>,
    pub implementations: Vec<Implementation>,
    pub usages: Vec<Usage>,
}

/// Trait for language-specific parsers
pub trait LanguageParser: Send + Sync {
    /// Parse symbols (functions, classes, etc.) from source code
    fn parse_symbols(&self, source: &str, file_path: &Path) -> Result<Vec<Symbol>>;

    /// Parse dependencies (imports, uses, etc.) from source code
    fn parse_dependencies(&self, source: &str, file_path: &Path) -> Result<Vec<Dependency>>;

    /// Parse function/method calls from source code
    fn parse_calls(&self, source: &str, file_path: &Path) -> Result<Vec<Call>> {
        // Default implementation returns empty - parsers can override
        let _ = (source, file_path);
        Ok(Vec::new())
    }

    /// Parse trait/interface implementations from source code
    fn parse_implementations(&self, source: &str, file_path: &Path) -> Result<Vec<Implementation>> {
        // Default implementation returns empty - parsers can override
        let _ = (source, file_path);
        Ok(Vec::new())
    }

    /// Parse symbol usages/references from source code
    fn parse_usages(&self, source: &str, file_path: &Path) -> Result<Vec<Usage>> {
        // Default implementation returns empty - parsers can override
        let _ = (source, file_path);
        Ok(Vec::new())
    }

    /// Parse all relationships from source code (convenience method)
    fn parse_all(&self, source: &str, file_path: &Path) -> Result<ParseResult> {
        Ok(ParseResult {
            symbols: self.parse_symbols(source, file_path)?,
            dependencies: self.parse_dependencies(source, file_path)?,
            calls: self.parse_calls(source, file_path)?,
            implementations: self.parse_implementations(source, file_path)?,
            usages: self.parse_usages(source, file_path)?,
        })
    }

    /// Get the language name
    fn language_name(&self) -> &'static str;

    /// Get file extensions this parser handles
    fn extensions(&self) -> &[&'static str];
}

/// Factory for creating language parsers based on file extension
pub struct ParserFactory;

impl ParserFactory {
    /// Get a parser for a file based on its extension
    pub fn for_file(path: &Path) -> Option<Box<dyn LanguageParser>> {
        let ext = path.extension()?.to_str()?;

        match ext {
            "rs" => Some(Box::new(rust::RustParser::new())),
            "ts" | "tsx" => Some(Box::new(typescript::TypeScriptParser::new(true))),
            "js" | "jsx" => Some(Box::new(typescript::TypeScriptParser::new(false))),
            "py" => Some(Box::new(python::PythonParser::new())),
            _ => None,
        }
    }

    /// Get a parser by language name
    pub fn for_language(language: &str) -> Option<Box<dyn LanguageParser>> {
        match language.to_lowercase().as_str() {
            "rust" | "rs" => Some(Box::new(rust::RustParser::new())),
            "typescript" | "ts" => Some(Box::new(typescript::TypeScriptParser::new(true))),
            "javascript" | "js" => Some(Box::new(typescript::TypeScriptParser::new(false))),
            "python" | "py" => Some(Box::new(python::PythonParser::new())),
            _ => None,
        }
    }

    /// Get all supported file extensions
    pub fn supported_extensions() -> Vec<&'static str> {
        vec!["rs", "ts", "tsx", "js", "jsx", "py"]
    }
}

/// Helper to extract text from a tree-sitter node
pub fn node_text<'a>(source: &'a str, node: &tree_sitter::Node) -> &'a str {
    &source[node.byte_range()]
}

/// Get the line number (1-indexed) for a byte offset
pub fn byte_to_line(source: &str, byte_offset: usize) -> usize {
    source[..byte_offset.min(source.len())]
        .chars()
        .filter(|&c| c == '\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_factory_for_file() {
        assert!(ParserFactory::for_file(Path::new("test.rs")).is_some());
        assert!(ParserFactory::for_file(Path::new("test.ts")).is_some());
        assert!(ParserFactory::for_file(Path::new("test.tsx")).is_some());
        assert!(ParserFactory::for_file(Path::new("test.js")).is_some());
        assert!(ParserFactory::for_file(Path::new("test.jsx")).is_some());
        assert!(ParserFactory::for_file(Path::new("test.py")).is_some());
        assert!(ParserFactory::for_file(Path::new("test.go")).is_none());
        assert!(ParserFactory::for_file(Path::new("test")).is_none());
    }

    #[test]
    fn test_parser_factory_for_language() {
        assert!(ParserFactory::for_language("rust").is_some());
        assert!(ParserFactory::for_language("Rust").is_some());
        assert!(ParserFactory::for_language("typescript").is_some());
        assert!(ParserFactory::for_language("javascript").is_some());
        assert!(ParserFactory::for_language("python").is_some());
        assert!(ParserFactory::for_language("go").is_none());
    }

    #[test]
    fn test_byte_to_line() {
        let source = "line1\nline2\nline3";
        assert_eq!(byte_to_line(source, 0), 1);
        assert_eq!(byte_to_line(source, 5), 1); // at newline
        assert_eq!(byte_to_line(source, 6), 2); // start of line2
        assert_eq!(byte_to_line(source, 12), 3); // start of line3
    }
}
