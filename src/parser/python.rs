//! Python language parser using tree-sitter

use crate::error::{CodeIndexError, Result};
use crate::indexer::{Dependency, DependencyKind, Symbol, SymbolKind, Visibility};
use crate::parser::{node_text, LanguageParser};
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Parser for Python source files
pub struct PythonParser;

impl PythonParser {
    pub fn new() -> Self {
        Self
    }

    fn create_parser(&self) -> Parser {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .expect("Failed to load Python grammar");
        parser
    }

    fn find_name_in_node<'a>(&self, node: &Node<'a>, source: &'a str) -> Option<&'a str> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return Some(node_text(source, &child));
            }
        }
        None
    }

    fn is_uppercase_constant(name: &str) -> bool {
        !name.is_empty() && name.chars().all(|c| c.is_uppercase() || c == '_' || c.is_numeric())
    }

    fn process_node(
        &self,
        node: Node,
        source: &str,
        file_path: &Path,
        parent_name: Option<&str>,
        symbols: &mut Vec<Symbol>,
    ) {
        let kind = node.kind();

        match kind {
            "function_definition" => {
                if let Some(name) = self.find_name_in_node(&node, source) {
                    let line_start = node.start_position().row + 1;
                    let line_end = node.end_position().row + 1;

                    let mut symbol = Symbol::new(
                        name,
                        SymbolKind::Function,
                        file_path,
                        line_start,
                        line_end,
                        "python",
                    )
                    .with_visibility(Visibility::Public);

                    if let Some(parent) = parent_name {
                        symbol = symbol.with_parent(parent);
                    }

                    // Try to extract signature
                    if let Some(params) = node.child_by_field_name("parameters") {
                        let params_text = node_text(source, &params);
                        let sig = format!("def {}{}", name, params_text);
                        symbol = symbol.with_signature(sig);
                    }

                    symbols.push(symbol);
                }
            }
            "class_definition" => {
                if let Some(name) = self.find_name_in_node(&node, source) {
                    let line_start = node.start_position().row + 1;
                    let line_end = node.end_position().row + 1;

                    let symbol = Symbol::new(
                        name,
                        SymbolKind::Class,
                        file_path,
                        line_start,
                        line_end,
                        "python",
                    )
                    .with_visibility(Visibility::Public);

                    symbols.push(symbol);

                    // Process class body with this class as parent
                    let class_name = name.to_string();
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "block" {
                            let mut block_cursor = child.walk();
                            for block_child in child.children(&mut block_cursor) {
                                self.process_node(
                                    block_child,
                                    source,
                                    file_path,
                                    Some(&class_name),
                                    symbols,
                                );
                            }
                        }
                    }
                    return; // Don't recurse normally since we handled the block
                }
            }
            "decorated_definition" => {
                // Process the actual definition inside (function or class)
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "function_definition" || child.kind() == "class_definition" {
                        self.process_node(child, source, file_path, parent_name, symbols);
                    }
                }
                return;
            }
            "expression_statement" => {
                // Check for top-level constant assignments (uppercase names)
                if parent_name.is_none() {
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "assignment" {
                            if let Some(left) = child.child_by_field_name("left") {
                                if left.kind() == "identifier" {
                                    let name = node_text(source, &left);
                                    if Self::is_uppercase_constant(name) {
                                        let line_start = node.start_position().row + 1;
                                        let line_end = node.end_position().row + 1;

                                        let symbol = Symbol::new(
                                            name,
                                            SymbolKind::Constant,
                                            file_path,
                                            line_start,
                                            line_end,
                                            "python",
                                        )
                                        .with_visibility(Visibility::Public);

                                        symbols.push(symbol);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.process_node(child, source, file_path, parent_name, symbols);
        }
    }

    fn process_dependencies(
        &self,
        node: Node,
        source: &str,
        file_path: &Path,
        deps: &mut Vec<Dependency>,
    ) {
        let kind = node.kind();

        match kind {
            "import_statement" => {
                // import os, sys
                let line_number = node.start_position().row + 1;
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "dotted_name" {
                        let module_name = node_text(source, &child);
                        let dep = Dependency::new(file_path, DependencyKind::Import, line_number)
                            .with_symbol(module_name);
                        deps.push(dep);
                    }
                    if child.kind() == "aliased_import" {
                        // import foo as bar
                        let mut alias_cursor = child.walk();
                        for alias_child in child.children(&mut alias_cursor) {
                            if alias_child.kind() == "dotted_name" {
                                let module_name = node_text(source, &alias_child);
                                let dep = Dependency::new(file_path, DependencyKind::Import, line_number)
                                    .with_symbol(module_name);
                                deps.push(dep);
                                break;
                            }
                        }
                    }
                }
            }
            "import_from_statement" => {
                // from typing import List, Dict
                let line_number = node.start_position().row + 1;

                // Get the module name
                let mut module_name = String::new();
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "dotted_name" {
                        module_name = node_text(source, &child).to_string();
                        break;
                    }
                    if child.kind() == "relative_import" {
                        module_name = node_text(source, &child).to_string();
                        break;
                    }
                }

                if !module_name.is_empty() {
                    let dep = Dependency::new(file_path, DependencyKind::Import, line_number)
                        .with_symbol(&module_name);
                    deps.push(dep);
                }
            }
            _ => {}
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.process_dependencies(child, source, file_path, deps);
        }
    }
}

impl Default for PythonParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageParser for PythonParser {
    fn parse_symbols(&self, source: &str, file_path: &Path) -> Result<Vec<Symbol>> {
        let mut parser = self.create_parser();

        let tree = parser.parse(source, None).ok_or_else(|| CodeIndexError::Parse {
            file: file_path.display().to_string(),
            message: "Failed to parse Python source".to_string(),
        })?;

        let mut symbols = Vec::new();
        self.process_node(tree.root_node(), source, file_path, None, &mut symbols);
        Ok(symbols)
    }

    fn parse_dependencies(&self, source: &str, file_path: &Path) -> Result<Vec<Dependency>> {
        let mut parser = self.create_parser();

        let tree = parser.parse(source, None).ok_or_else(|| CodeIndexError::Parse {
            file: file_path.display().to_string(),
            message: "Failed to parse Python source".to_string(),
        })?;

        let mut deps = Vec::new();
        self.process_dependencies(tree.root_node(), source, file_path, &mut deps);
        Ok(deps)
    }

    fn language_name(&self) -> &'static str {
        "python"
    }

    fn extensions(&self) -> &[&'static str] {
        &["py"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_function() {
        let parser = PythonParser::new();
        let source = r#"
def greet(name: str) -> str:
    """Greet someone."""
    return f"Hello, {name}!"
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.py"))
            .unwrap();

        assert_eq!(symbols.len(), 1, "Should find 1 function");
        assert_eq!(symbols[0].name, "greet");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
    }

    #[test]
    fn test_parse_class() {
        let parser = PythonParser::new();
        let source = r#"
class User:
    """A user class."""

    def __init__(self, name: str):
        self.name = name

    def greet(self) -> str:
        return f"Hello, {self.name}!"
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.py"))
            .unwrap();

        // Should find: User class, __init__, greet
        let class_sym = symbols.iter().find(|s| s.name == "User");
        assert!(class_sym.is_some(), "Should find User class");
        assert_eq!(class_sym.unwrap().kind, SymbolKind::Class);

        let init_sym = symbols.iter().find(|s| s.name == "__init__");
        assert!(init_sym.is_some(), "Should find __init__ method");
        assert_eq!(init_sym.unwrap().parent, Some("User".to_string()));
    }

    #[test]
    fn test_parse_async_function() {
        let parser = PythonParser::new();
        let source = r#"
async def fetch_data(url: str) -> dict:
    async with aiohttp.ClientSession() as session:
        async with session.get(url) as response:
            return await response.json()
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.py"))
            .unwrap();

        assert_eq!(symbols.len(), 1, "Should find 1 async function");
        assert_eq!(symbols[0].name, "fetch_data");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
    }

    #[test]
    fn test_parse_decorated_function() {
        let parser = PythonParser::new();
        let source = r#"
@staticmethod
def helper():
    pass

@property
def name(self):
    return self._name
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.py"))
            .unwrap();

        assert!(symbols.len() >= 2, "Should find at least 2 functions");
        assert!(symbols.iter().any(|s| s.name == "helper"));
        assert!(symbols.iter().any(|s| s.name == "name"));
    }

    #[test]
    fn test_parse_imports() {
        let parser = PythonParser::new();
        let source = r#"
import os
import sys
from typing import List, Dict
from pathlib import Path
from . import utils
from ..models import User
"#;
        let deps = parser
            .parse_dependencies(source, Path::new("/test.py"))
            .unwrap();

        assert!(deps.len() >= 4, "Should find at least 4 imports");
        assert!(deps.iter().all(|d| d.kind == DependencyKind::Import));

        // Check specific imports
        let symbols: Vec<_> = deps.iter().filter_map(|d| d.symbol_name.as_ref()).collect();
        assert!(symbols.iter().any(|s| s.contains("os")));
        assert!(symbols.iter().any(|s| s.contains("typing")));
    }

    #[test]
    fn test_parse_constant() {
        let parser = PythonParser::new();
        let source = r#"
MAX_SIZE = 1024
PI = 3.14159
DEBUG = True
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.py"))
            .unwrap();

        // Constants (uppercase assignments at module level)
        assert!(symbols.iter().any(|s| s.name == "MAX_SIZE"), "Should find MAX_SIZE");
        assert!(symbols.iter().any(|s| s.name == "PI"), "Should find PI");
    }

    #[test]
    fn test_parser_factory_python() {
        use crate::parser::ParserFactory;

        assert!(ParserFactory::for_file(Path::new("test.py")).is_some());
        assert!(ParserFactory::for_language("python").is_some());
    }
}
