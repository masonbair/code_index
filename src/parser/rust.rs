//! Rust language parser using tree-sitter

use crate::error::{CodeIndexError, Result};
use crate::indexer::{Call, Dependency, DependencyKind, Implementation, Symbol, SymbolKind, Usage, UsageKind, Visibility};
use crate::parser::{node_text, LanguageParser};
use std::path::Path;
use tree_sitter::{Node, Parser};

/// Parser for Rust source files
pub struct RustParser;

impl RustParser {
    pub fn new() -> Self {
        Self
    }

    fn extract_visibility(&self, node: &Node, source: &str) -> Visibility {
        // Look for visibility_modifier child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                let vis_text = node_text(source, &child);
                if vis_text.starts_with("pub(crate)") {
                    return Visibility::Internal;
                } else if vis_text.starts_with("pub(super)") || vis_text.starts_with("pub(in") {
                    return Visibility::Protected;
                } else if vis_text.starts_with("pub") {
                    return Visibility::Public;
                }
            }
        }
        Visibility::Private
    }

    fn extract_function_signature(&self, node: &Node, source: &str) -> Option<String> {
        let mut sig_parts = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "visibility_modifier" => {
                    sig_parts.push(node_text(source, &child).to_string());
                }
                "function_modifiers" => {
                    sig_parts.push(node_text(source, &child).to_string());
                }
                "name" | "identifier" => {
                    sig_parts.push(format!("fn {}", node_text(source, &child)));
                }
                "type_parameters" | "parameters" => {
                    sig_parts.push(node_text(source, &child).to_string());
                }
                "return_type" => {
                    sig_parts.push(node_text(source, &child).to_string());
                }
                "block" => break, // Stop at function body
                _ => {}
            }
        }

        if sig_parts.is_empty() {
            None
        } else {
            Some(sig_parts.join(" ").replace("  ", " ").trim().to_string())
        }
    }

    fn find_name_in_node<'a>(&self, node: &Node<'a>, source: &'a str) -> Option<&'a str> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "name" || child.kind() == "identifier" || child.kind() == "type_identifier" {
                return Some(node_text(source, &child));
            }
        }
        None
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

        let symbol_info: Option<(SymbolKind, bool)> = match kind {
            "function_item" => Some((SymbolKind::Function, true)),
            "struct_item" => Some((SymbolKind::Struct, false)),
            "enum_item" => Some((SymbolKind::Enum, false)),
            "trait_item" => Some((SymbolKind::Trait, false)),
            "impl_item" => Some((SymbolKind::Impl, false)),
            "type_item" => Some((SymbolKind::Type, false)),
            "const_item" => Some((SymbolKind::Constant, false)),
            "static_item" => Some((SymbolKind::Variable, false)),
            "mod_item" => Some((SymbolKind::Module, false)),
            _ => None,
        };

        if let Some((symbol_kind, is_function)) = symbol_info {
            if let Some(name) = self.find_name_in_node(&node, source) {
                let line_start = node.start_position().row + 1;
                let line_end = node.end_position().row + 1;
                let visibility = self.extract_visibility(&node, source);

                let mut symbol = Symbol::new(name, symbol_kind, file_path, line_start, line_end, "rust")
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

                // For impl blocks, process children with this as parent
                if symbol_kind == SymbolKind::Impl || symbol_kind == SymbolKind::Trait {
                    let impl_name = name.to_string();
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        self.process_node(child, source, file_path, Some(&impl_name), symbols);
                    }
                    return;
                }
            }
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

        if kind == "use_declaration" {
            let line_number = node.start_position().row + 1;

            // Extract the use path
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "use_wildcard"
                    || child.kind() == "use_list"
                    || child.kind() == "scoped_identifier"
                    || child.kind() == "identifier"
                    || child.kind() == "scoped_use_list"
                {
                    let use_path = node_text(source, &child);
                    let dep = Dependency::new(file_path, DependencyKind::Import, line_number)
                        .with_symbol(use_path);
                    deps.push(dep);
                    break;
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.process_dependencies(child, source, file_path, deps);
        }
    }

    fn process_calls(
        &self,
        node: Node,
        source: &str,
        file_path: &Path,
        current_function: Option<&str>,
        calls: &mut Vec<Call>,
    ) {
        let kind = node.kind();

        // Track current function context
        let mut new_function_name = current_function;
        if kind == "function_item" {
            if let Some(name) = self.find_name_in_node(&node, source) {
                new_function_name = Some(name);
            }
        }

        // Detect function/method calls
        if kind == "call_expression" {
            let line = node.start_position().row + 1;
            let column = node.start_position().column + 1;

            // Get the function being called
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    // Simple function call: foo()
                    let callee_name = node_text(source, &child);
                    let caller_name = new_function_name.unwrap_or("<global>");
                    calls.push(Call::new(caller_name, callee_name, file_path, line, column));
                    break;
                } else if child.kind() == "scoped_identifier" {
                    // Type::method() call
                    let callee_name = node_text(source, &child);
                    let caller_name = new_function_name.unwrap_or("<global>");
                    calls.push(Call::new(caller_name, callee_name, file_path, line, column));
                    break;
                } else if child.kind() == "field_expression" {
                    // Method call: obj.method()
                    if let Some(method_name) = self.extract_method_name(&child, source) {
                        let caller_name = new_function_name.unwrap_or("<global>");
                        calls.push(
                            Call::new(caller_name, method_name, file_path, line, column)
                                .with_method(true),
                        );
                    }
                    break;
                }
            }
        }

        // Detect await expressions (async calls)
        if kind == "await_expression" {
            let line = node.start_position().row + 1;

            // The expression being awaited is the first child
            let mut cursor = node.walk();
            let first_child = node.children(&mut cursor).next();
            drop(cursor);

            if let Some(child) = first_child {
                if child.kind() == "call_expression" {
                    // Already handled above, but mark as async
                    if let Some(last_call) = calls.last_mut() {
                        if last_call.call_line == line {
                            last_call.is_async = true;
                        }
                    }
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.process_calls(child, source, file_path, new_function_name, calls);
        }
    }

    fn extract_method_name<'a>(&self, field_expr: &Node<'a>, source: &'a str) -> Option<&'a str> {
        // field_expression has structure: receiver.field
        // We want the field name (method being called)
        let mut cursor = field_expr.walk();
        for child in field_expr.children(&mut cursor) {
            if child.kind() == "field_identifier" {
                return Some(node_text(source, &child));
            }
        }
        None
    }

    fn process_implementations(
        &self,
        node: Node,
        source: &str,
        file_path: &Path,
        impls: &mut Vec<Implementation>,
    ) {
        let kind = node.kind();

        if kind == "impl_item" {
            let line = node.start_position().row + 1;

            let mut type_name: Option<&str> = None;
            let mut trait_name: Option<&str> = None;

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "type_identifier" | "generic_type" => {
                        let name = node_text(source, &child);
                        if trait_name.is_none() && type_name.is_some() {
                            // This is "impl Trait for Type" pattern
                            trait_name = type_name;
                            type_name = Some(name);
                        } else {
                            type_name = Some(name);
                        }
                    }
                    "scoped_type_identifier" => {
                        // Handle path::Type
                        type_name = Some(node_text(source, &child));
                    }
                    _ => {}
                }
            }

            // Check if this is a trait impl (impl Trait for Type)
            let impl_text = node_text(source, &node);
            if impl_text.contains(" for ") {
                if let (Some(implementor), Some(trait_n)) = (type_name, trait_name) {
                    impls.push(Implementation::new(implementor, trait_n, file_path, line));
                } else if let Some(implementor) = type_name {
                    // Try to extract trait name from the impl text
                    if let Some(start) = impl_text.find("impl ") {
                        if let Some(for_pos) = impl_text.find(" for ") {
                            let trait_part = &impl_text[start + 5..for_pos];
                            let trait_part = trait_part.trim();
                            // Handle generics by taking only the trait name
                            let trait_n = trait_part.split('<').next().unwrap_or(trait_part);
                            impls.push(Implementation::new(implementor, trait_n, file_path, line));
                        }
                    }
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.process_implementations(child, source, file_path, impls);
        }
    }

    fn process_usages(
        &self,
        node: Node,
        source: &str,
        file_path: &Path,
        current_function: Option<&str>,
        usages: &mut Vec<Usage>,
    ) {
        let kind = node.kind();

        // Track current function context
        let mut new_function_name = current_function;
        if kind == "function_item" {
            if let Some(name) = self.find_name_in_node(&node, source) {
                new_function_name = Some(name);
            }
        }

        // Type annotations (e.g., x: Foo)
        if kind == "type_identifier" {
            // Check parent to determine usage kind
            if let Some(parent) = node.parent() {
                let parent_kind = parent.kind();
                let usage_kind = match parent_kind {
                    "type_annotation" | "type_cast_expression" => UsageKind::TypeAnnotation,
                    "function_return_type" | "return_type" => UsageKind::ReturnType,
                    "type_bound" | "trait_bound" => UsageKind::TraitBound,
                    "type_arguments" | "type_parameters" | "generic_type" => UsageKind::GenericParam,
                    _ => UsageKind::TypeAnnotation,
                };

                let type_name = node_text(source, &node);
                // Skip primitive types and common std types
                if !["i32", "i64", "u32", "u64", "f32", "f64", "bool", "str", "String", "usize", "isize", "char", "u8", "i8", "u16", "i16"].contains(&type_name) {
                    let line = node.start_position().row + 1;
                    let column = node.start_position().column + 1;
                    let mut usage = Usage::new(type_name, usage_kind, file_path, line, column);
                    if let Some(fn_name) = new_function_name {
                        usage = usage.with_context(fn_name);
                    }
                    usages.push(usage);
                }
            }
        }

        // Field access (e.g., x.field)
        if kind == "field_expression" {
            if let Some(field) = self.extract_method_name(&node, source) {
                let line = node.start_position().row + 1;
                let column = node.start_position().column + 1;
                let mut usage = Usage::new(field, UsageKind::FieldAccess, file_path, line, column);
                if let Some(fn_name) = new_function_name {
                    usage = usage.with_context(fn_name);
                }
                usages.push(usage);
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.process_usages(child, source, file_path, new_function_name, usages);
        }
    }
}

impl Default for RustParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageParser for RustParser {
    fn parse_symbols(&self, source: &str, file_path: &Path) -> Result<Vec<Symbol>> {
        // Need mutable borrow for parsing
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("Failed to load Rust grammar");

        let tree = parser.parse(source, None).ok_or_else(|| CodeIndexError::Parse {
            file: file_path.display().to_string(),
            message: "Failed to parse Rust source".to_string(),
        })?;

        let mut symbols = Vec::new();
        self.process_node(tree.root_node(), source, file_path, None, &mut symbols);
        Ok(symbols)
    }

    fn parse_dependencies(&self, source: &str, file_path: &Path) -> Result<Vec<Dependency>> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("Failed to load Rust grammar");

        let tree = parser.parse(source, None).ok_or_else(|| CodeIndexError::Parse {
            file: file_path.display().to_string(),
            message: "Failed to parse Rust source".to_string(),
        })?;

        let mut deps = Vec::new();
        self.process_dependencies(tree.root_node(), source, file_path, &mut deps);
        Ok(deps)
    }

    fn parse_calls(&self, source: &str, file_path: &Path) -> Result<Vec<Call>> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("Failed to load Rust grammar");

        let tree = parser.parse(source, None).ok_or_else(|| CodeIndexError::Parse {
            file: file_path.display().to_string(),
            message: "Failed to parse Rust source".to_string(),
        })?;

        let mut calls = Vec::new();
        self.process_calls(tree.root_node(), source, file_path, None, &mut calls);
        Ok(calls)
    }

    fn parse_implementations(&self, source: &str, file_path: &Path) -> Result<Vec<Implementation>> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("Failed to load Rust grammar");

        let tree = parser.parse(source, None).ok_or_else(|| CodeIndexError::Parse {
            file: file_path.display().to_string(),
            message: "Failed to parse Rust source".to_string(),
        })?;

        let mut impls = Vec::new();
        self.process_implementations(tree.root_node(), source, file_path, &mut impls);
        Ok(impls)
    }

    fn parse_usages(&self, source: &str, file_path: &Path) -> Result<Vec<Usage>> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("Failed to load Rust grammar");

        let tree = parser.parse(source, None).ok_or_else(|| CodeIndexError::Parse {
            file: file_path.display().to_string(),
            message: "Failed to parse Rust source".to_string(),
        })?;

        let mut usages = Vec::new();
        self.process_usages(tree.root_node(), source, file_path, None, &mut usages);
        Ok(usages)
    }

    fn language_name(&self) -> &'static str {
        "rust"
    }

    fn extensions(&self) -> &[&'static str] {
        &["rs"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_function() {
        let parser = RustParser::new();
        let source = r#"
pub fn hello_world() -> String {
    "Hello, world!".to_string()
}
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.rs"))
            .unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "hello_world");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[0].visibility, Visibility::Public);
        assert!(symbols[0].signature.is_some());
    }

    #[test]
    fn test_parse_struct() {
        let parser = RustParser::new();
        let source = r#"
pub struct User {
    name: String,
    age: u32,
}
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.rs"))
            .unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "User");
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
        assert_eq!(symbols[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_parse_enum() {
        let parser = RustParser::new();
        let source = r#"
pub enum Status {
    Active,
    Inactive,
}
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.rs"))
            .unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Status");
        assert_eq!(symbols[0].kind, SymbolKind::Enum);
    }

    #[test]
    fn test_parse_impl_methods() {
        let parser = RustParser::new();
        let source = r#"
impl User {
    pub fn new(name: String) -> Self {
        Self { name, age: 0 }
    }

    fn private_method(&self) {}
}
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.rs"))
            .unwrap();

        // Should find: impl, new, private_method
        assert!(symbols.len() >= 2);

        let new_fn = symbols.iter().find(|s| s.name == "new").unwrap();
        assert_eq!(new_fn.kind, SymbolKind::Function);
        assert_eq!(new_fn.visibility, Visibility::Public);
        assert_eq!(new_fn.parent, Some("User".to_string()));

        let priv_fn = symbols.iter().find(|s| s.name == "private_method").unwrap();
        assert_eq!(priv_fn.visibility, Visibility::Private);
    }

    #[test]
    fn test_parse_use_statements() {
        let parser = RustParser::new();
        let source = r#"
use std::collections::HashMap;
use crate::error::Result;
use super::config::Config;
"#;
        let deps = parser
            .parse_dependencies(source, Path::new("/test.rs"))
            .unwrap();

        assert_eq!(deps.len(), 3);
        assert!(deps.iter().all(|d| d.kind == DependencyKind::Import));
    }

    #[test]
    fn test_parse_trait() {
        let parser = RustParser::new();
        let source = r#"
pub trait Drawable {
    fn draw(&self);
    fn resize(&mut self, width: u32, height: u32);
}
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.rs"))
            .unwrap();

        // Should at least find the trait itself
        let trait_sym = symbols.iter().find(|s| s.name == "Drawable").unwrap();
        assert_eq!(trait_sym.kind, SymbolKind::Trait);
        assert_eq!(trait_sym.visibility, Visibility::Public);

        // Note: trait method declarations (without body) may not be extracted as full symbols
        // This is a limitation of the current parser - it only extracts function_item nodes
    }

    #[test]
    fn test_parse_const_static() {
        let parser = RustParser::new();
        let source = r#"
pub const MAX_SIZE: usize = 1024;
static mut COUNTER: u32 = 0;
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.rs"))
            .unwrap();

        let const_sym = symbols.iter().find(|s| s.name == "MAX_SIZE").unwrap();
        assert_eq!(const_sym.kind, SymbolKind::Constant);

        let static_sym = symbols.iter().find(|s| s.name == "COUNTER").unwrap();
        assert_eq!(static_sym.kind, SymbolKind::Variable);
    }

    #[test]
    fn test_parse_module() {
        let parser = RustParser::new();
        let source = r#"
pub mod config {
    pub fn load() {}
}
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.rs"))
            .unwrap();

        let mod_sym = symbols.iter().find(|s| s.name == "config").unwrap();
        assert_eq!(mod_sym.kind, SymbolKind::Module);
    }

    #[test]
    fn test_visibility_levels() {
        let parser = RustParser::new();
        let source = r#"
pub fn public_fn() {}
pub(crate) fn crate_fn() {}
pub(super) fn super_fn() {}
fn private_fn() {}
"#;
        let symbols = parser
            .parse_symbols(source, Path::new("/test.rs"))
            .unwrap();

        let public = symbols.iter().find(|s| s.name == "public_fn").unwrap();
        assert_eq!(public.visibility, Visibility::Public);

        let crate_vis = symbols.iter().find(|s| s.name == "crate_fn").unwrap();
        assert_eq!(crate_vis.visibility, Visibility::Internal);

        let super_vis = symbols.iter().find(|s| s.name == "super_fn").unwrap();
        assert_eq!(super_vis.visibility, Visibility::Protected);

        let private = symbols.iter().find(|s| s.name == "private_fn").unwrap();
        assert_eq!(private.visibility, Visibility::Private);
    }
}
