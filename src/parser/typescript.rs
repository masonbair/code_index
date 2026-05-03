//! TypeScript/JavaScript language parser using tree-sitter

use crate::error::{CodeIndexError, Result};
use crate::indexer::{Dependency, DependencyKind, Symbol, SymbolKind, Visibility};
use crate::parser::{node_text, LanguageParser};
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Parser for TypeScript and JavaScript source files
pub struct TypeScriptParser {
    is_typescript: bool,
}

impl TypeScriptParser {
    pub fn new(is_typescript: bool) -> Self {
        Self { is_typescript }
    }

    fn create_parser(&self) -> Parser {
        let mut parser = Parser::new();
        if self.is_typescript {
            parser
                .set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())
                .expect("Failed to load TypeScript grammar");
        } else {
            parser
                .set_language(&tree_sitter_javascript::LANGUAGE.into())
                .expect("Failed to load JavaScript grammar");
        }
        parser
    }

    fn extract_visibility(&self, node: &Node, source: &str) -> Visibility {
        // Check for export keyword (public) or private/protected modifiers
        let parent = node.parent();

        // Check if this is part of an export statement
        if let Some(p) = parent {
            if p.kind() == "export_statement" {
                return Visibility::Public;
            }
        }

        // Check for TypeScript visibility modifiers
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind == "accessibility_modifier" {
                let text = node_text(source, &child);
                match text {
                    "public" => return Visibility::Public,
                    "private" => return Visibility::Private,
                    "protected" => return Visibility::Protected,
                    _ => {}
                }
            }
        }

        // Default to private (not exported)
        Visibility::Private
    }

    fn find_name_in_node<'a>(&self, node: &Node<'a>, source: &'a str) -> Option<&'a str> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind == "identifier" || kind == "property_identifier" || kind == "type_identifier" {
                return Some(node_text(source, &child));
            }
        }
        None
    }

    fn extract_function_signature(&self, node: &Node, source: &str) -> Option<String> {
        // Get text from start of node to just before the body
        let mut cursor = node.walk();
        let mut end_byte = node.end_byte();

        for child in node.children(&mut cursor) {
            if child.kind() == "statement_block" || child.kind() == "block" {
                end_byte = child.start_byte();
                break;
            }
        }

        let sig = &source[node.start_byte()..end_byte];
        Some(sig.trim().to_string())
    }

    fn process_node(
        &self,
        node: Node,
        source: &str,
        file_path: &Path,
        parent_name: Option<&str>,
        symbols: &mut Vec<Symbol>,
        in_export: bool,
    ) {
        let kind = node.kind();

        // Handle export statements by passing the export flag down
        if kind == "export_statement" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.process_node(child, source, file_path, parent_name, symbols, true);
            }
            return;
        }

        let symbol_info: Option<(SymbolKind, bool)> = match kind {
            "function_declaration" | "function" => Some((SymbolKind::Function, true)),
            "arrow_function" => Some((SymbolKind::Function, true)),
            "class_declaration" | "class" => Some((SymbolKind::Class, false)),
            "interface_declaration" => Some((SymbolKind::Interface, false)),
            "type_alias_declaration" => Some((SymbolKind::Type, false)),
            "enum_declaration" => Some((SymbolKind::Enum, false)),
            "method_definition" => Some((SymbolKind::Method, true)),
            "public_field_definition" | "field_definition" => Some((SymbolKind::Variable, false)),
            "lexical_declaration" | "variable_declaration" => {
                // Handle const/let/var declarations
                self.process_variable_declaration(node, source, file_path, symbols, in_export);
                return;
            }
            _ => None,
        };

        if let Some((symbol_kind, is_function)) = symbol_info {
            if let Some(name) = self.find_name_in_node(&node, source) {
                let line_start = node.start_position().row + 1;
                let line_end = node.end_position().row + 1;

                let visibility = if in_export {
                    Visibility::Public
                } else {
                    self.extract_visibility(&node, source)
                };

                let lang = if self.is_typescript {
                    "typescript"
                } else {
                    "javascript"
                };

                let mut symbol =
                    Symbol::new(name, symbol_kind, file_path, line_start, line_end, lang)
                        .with_visibility(visibility);

                if is_function {
                    if let Some(sig) = self.extract_function_signature(&node, source) {
                        symbol = symbol.with_signature(sig);
                    }
                }

                if let Some(parent) = parent_name {
                    symbol = symbol.with_parent(parent);
                }

                symbols.push(symbol);

                // For classes/interfaces, process children with this as parent
                if symbol_kind == SymbolKind::Class || symbol_kind == SymbolKind::Interface {
                    let class_name = name.to_string();
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "class_body" || child.kind() == "interface_body"
                            || child.kind() == "object_type"
                        {
                            let mut body_cursor = child.walk();
                            for body_child in child.children(&mut body_cursor) {
                                self.process_node(
                                    body_child,
                                    source,
                                    file_path,
                                    Some(&class_name),
                                    symbols,
                                    false,
                                );
                            }
                        }
                    }
                    return;
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.process_node(child, source, file_path, parent_name, symbols, in_export);
        }
    }

    fn process_variable_declaration(
        &self,
        node: Node,
        source: &str,
        file_path: &Path,
        symbols: &mut Vec<Symbol>,
        in_export: bool,
    ) {
        let is_const = node.kind() == "lexical_declaration"
            && node
                .children(&mut node.walk())
                .any(|c| c.kind() == "const");

        let symbol_kind = if is_const {
            SymbolKind::Constant
        } else {
            SymbolKind::Variable
        };

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                if let Some(name) = self.find_name_in_node(&child, source) {
                    let line_start = node.start_position().row + 1;
                    let line_end = node.end_position().row + 1;

                    let visibility = if in_export {
                        Visibility::Public
                    } else {
                        Visibility::Private
                    };

                    let lang = if self.is_typescript {
                        "typescript"
                    } else {
                        "javascript"
                    };

                    // Check if this is a function expression or arrow function
                    let mut var_cursor = child.walk();
                    let is_function = child.children(&mut var_cursor).any(|c| {
                        c.kind() == "arrow_function" || c.kind() == "function"
                    });

                    let actual_kind = if is_function {
                        SymbolKind::Function
                    } else {
                        symbol_kind
                    };

                    let symbol =
                        Symbol::new(name, actual_kind, file_path, line_start, line_end, lang)
                            .with_visibility(visibility);

                    symbols.push(symbol);
                }
            }
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

        if kind == "import_statement" {
            let line_number = node.start_position().row + 1;

            // Find the import source
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "string" || child.kind() == "string_literal" {
                    let import_path = node_text(source, &child)
                        .trim_matches('"')
                        .trim_matches('\'');

                    let dep = Dependency::new(file_path, DependencyKind::Import, line_number)
                        .with_symbol(import_path);
                    deps.push(dep);
                    break;
                }
            }
        } else if kind == "call_expression" {
            // Handle require() calls
            let mut cursor = node.walk();
            let mut is_require = false;
            let mut require_path = None;

            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" && node_text(source, &child) == "require" {
                    is_require = true;
                }
                if child.kind() == "arguments" {
                    let mut arg_cursor = child.walk();
                    for arg in child.children(&mut arg_cursor) {
                        if arg.kind() == "string" || arg.kind() == "string_literal" {
                            require_path = Some(
                                node_text(source, &arg)
                                    .trim_matches('"')
                                    .trim_matches('\'')
                                    .to_string(),
                            );
                            break;
                        }
                    }
                }
            }

            if is_require {
                if let Some(path) = require_path {
                    let line_number = node.start_position().row + 1;
                    let dep = Dependency::new(file_path, DependencyKind::Import, line_number)
                        .with_symbol(&path);
                    deps.push(dep);
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.process_dependencies(child, source, file_path, deps);
        }
    }
}

impl LanguageParser for TypeScriptParser {
    fn parse_symbols(&self, source: &str, file_path: &Path) -> Result<Vec<Symbol>> {
        let mut parser = self.create_parser();

        let tree = parser.parse(source, None).ok_or_else(|| CodeIndexError::Parse {
            file: file_path.display().to_string(),
            message: "Failed to parse TypeScript/JavaScript source".to_string(),
        })?;

        let mut symbols = Vec::new();
        self.process_node(tree.root_node(), source, file_path, None, &mut symbols, false);
        Ok(symbols)
    }

    fn parse_dependencies(&self, source: &str, file_path: &Path) -> Result<Vec<Dependency>> {
        let mut parser = self.create_parser();

        let tree = parser.parse(source, None).ok_or_else(|| CodeIndexError::Parse {
            file: file_path.display().to_string(),
            message: "Failed to parse TypeScript/JavaScript source".to_string(),
        })?;

        let mut deps = Vec::new();
        self.process_dependencies(tree.root_node(), source, file_path, &mut deps);
        Ok(deps)
    }

    fn language_name(&self) -> &'static str {
        if self.is_typescript {
            "typescript"
        } else {
            "javascript"
        }
    }

    fn extensions(&self) -> &[&'static str] {
        if self.is_typescript {
            &["ts", "tsx"]
        } else {
            &["js", "jsx"]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_function_declaration() {
        let parser = TypeScriptParser::new(true);
        let source = r#"
export function greet(name: string): string {
    return `Hello, ${name}!`;
}
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.ts"))
            .unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "greet");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_parse_arrow_function() {
        let parser = TypeScriptParser::new(true);
        let source = r#"
export const add = (a: number, b: number): number => a + b;
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.ts"))
            .unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "add");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_parse_class() {
        let parser = TypeScriptParser::new(true);
        let source = r#"
export class User {
    private name: string;

    constructor(name: string) {
        this.name = name;
    }

    public greet(): string {
        return `Hello, ${this.name}!`;
    }
}
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.ts"))
            .unwrap();

        let class = symbols.iter().find(|s| s.name == "User").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
        assert_eq!(class.visibility, Visibility::Public);

        // Methods should have User as parent
        let greet = symbols.iter().find(|s| s.name == "greet");
        if let Some(g) = greet {
            assert_eq!(g.parent, Some("User".to_string()));
        }
    }

    #[test]
    fn test_parse_interface() {
        let parser = TypeScriptParser::new(true);
        let source = r#"
export interface Config {
    host: string;
    port: number;
}
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.ts"))
            .unwrap();

        let iface = symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(iface.kind, SymbolKind::Interface);
        assert_eq!(iface.visibility, Visibility::Public);
    }

    #[test]
    fn test_parse_type_alias() {
        let parser = TypeScriptParser::new(true);
        let source = r#"
export type UserId = string | number;
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.ts"))
            .unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "UserId");
        assert_eq!(symbols[0].kind, SymbolKind::Type);
    }

    #[test]
    fn test_parse_enum() {
        let parser = TypeScriptParser::new(true);
        let source = r#"
export enum Status {
    Active = "active",
    Inactive = "inactive",
}
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.ts"))
            .unwrap();

        let enum_sym = symbols.iter().find(|s| s.name == "Status").unwrap();
        assert_eq!(enum_sym.kind, SymbolKind::Enum);
    }

    #[test]
    fn test_parse_const_variable() {
        let parser = TypeScriptParser::new(true);
        let source = r#"
export const MAX_SIZE = 1024;
let counter = 0;
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.ts"))
            .unwrap();

        let const_sym = symbols.iter().find(|s| s.name == "MAX_SIZE").unwrap();
        assert_eq!(const_sym.kind, SymbolKind::Constant);
        assert_eq!(const_sym.visibility, Visibility::Public);

        let var_sym = symbols.iter().find(|s| s.name == "counter").unwrap();
        assert_eq!(var_sym.kind, SymbolKind::Variable);
        assert_eq!(var_sym.visibility, Visibility::Private);
    }

    #[test]
    fn test_parse_imports() {
        let parser = TypeScriptParser::new(true);
        let source = r#"
import { User } from "./models/user";
import * as utils from "../utils";
import express from "express";
"#;
        let deps = parser
            .parse_dependencies(source, Path::new("/test.ts"))
            .unwrap();

        assert_eq!(deps.len(), 3);
        assert!(deps.iter().all(|d| d.kind == DependencyKind::Import));

        let paths: Vec<_> = deps.iter().filter_map(|d| d.symbol_name.as_ref()).collect();
        assert!(paths.contains(&&"./models/user".to_string()));
        assert!(paths.contains(&&"../utils".to_string()));
        assert!(paths.contains(&&"express".to_string()));
    }

    #[test]
    fn test_parse_require() {
        let parser = TypeScriptParser::new(false);
        let source = r#"
const express = require("express");
const path = require("path");
"#;
        let deps = parser
            .parse_dependencies(source, Path::new("/test.js"))
            .unwrap();

        assert_eq!(deps.len(), 2);
        let paths: Vec<_> = deps.iter().filter_map(|d| d.symbol_name.as_ref()).collect();
        assert!(paths.contains(&&"express".to_string()));
        assert!(paths.contains(&&"path".to_string()));
    }

    #[test]
    fn test_javascript_function() {
        let parser = TypeScriptParser::new(false);
        let source = r#"
function processData(data) {
    return data.map(x => x * 2);
}
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.js"))
            .unwrap();

        assert!(symbols.len() >= 1);
        let func = symbols.iter().find(|s| s.name == "processData").unwrap();
        assert_eq!(func.kind, SymbolKind::Function);
        assert_eq!(func.language, "javascript");
    }
}
