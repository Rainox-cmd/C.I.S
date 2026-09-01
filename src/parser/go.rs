use crate::parser::{Import, LanguageParser, ParseResult, Symbol, SymbolKind};
use std::path::Path;

pub struct GoParser;

impl LanguageParser for GoParser {
    fn language(&self) -> &str {
        "Go"
    }

    fn parse(&self, _path: &Path, content: &str) -> ParseResult {
        let mut result = ParseResult {
            symbols: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            syntax_ok: true,
            syntax_error: None,
            language: self.language().to_string(),
        };

        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(tree_sitter_go::language()).is_err() {
            result.syntax_ok = false;
            result.syntax_error = Some("Failed to load Go grammar".to_string());
            return result;
        }

        let tree = match parser.parse(content, None) {
            Some(t) => t,
            None => {
                result.syntax_ok = false;
                result.syntax_error = Some("Failed to parse content".to_string());
                return result;
            }
        };

        if tree.root_node().has_error() {
            result.syntax_ok = false;
            result.syntax_error = Some("Syntax error detected".to_string());
        }

        let mut stack = vec![tree.root_node()];

        while let Some(node) = stack.pop() {
            let kind = node.kind();
            
            if kind == "function_declaration" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                        result.symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Function,
                            line: (name_node.start_position().row + 1) as u32,
                            column: name_node.start_position().column as u32,
                        });
                    }
                }
            } else if kind == "method_declaration" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                        result.symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Method,
                            line: (name_node.start_position().row + 1) as u32,
                            column: name_node.start_position().column as u32,
                        });
                    }
                }
            } else if kind == "type_declaration" {
                for i in 0..node.child_count() {
                    if let Some(spec) = node.child(i) {
                        if spec.kind() == "type_spec" {
                            if let Some(name_node) = spec.child_by_field_name("name") {
                                if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                                    let mut sym_kind = SymbolKind::Other("Type".to_string());
                                    if let Some(type_node) = spec.child_by_field_name("type") {
                                        if type_node.kind() == "struct_type" {
                                            sym_kind = SymbolKind::Struct;
                                        } else if type_node.kind() == "interface_type" {
                                            sym_kind = SymbolKind::Interface;
                                        }
                                    }
                                    result.symbols.push(Symbol {
                                        name: name.to_string(),
                                        kind: sym_kind,
                                        line: (name_node.start_position().row + 1) as u32,
                                        column: name_node.start_position().column as u32,
                                    });
                                }
                            }
                        }
                    }
                }
            } else if kind == "import_spec" {
                if let Some(path_node) = node.child_by_field_name("path") {
                    if let Ok(path_quoted) = path_node.utf8_text(content.as_bytes()) {
                        let path = path_quoted.trim_matches(&['\'', '"'][..]).to_string();
                        let is_rel = path.starts_with('.') || path.starts_with('/');
                        result.imports.push(Import {
                            path,
                            is_relative: is_rel,
                            line: (path_node.start_position().row + 1) as u32,
                            column: path_node.start_position().column as u32,
                        });
                    }
                }
            }

            for i in (0..node.child_count()).rev() {
                if let Some(child) = node.child(i) {
                    stack.push(child);
                }
            }
        }

        result
    }
}
