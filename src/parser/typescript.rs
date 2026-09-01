use crate::parser::{Import, LanguageParser, ParseResult, Symbol, SymbolKind};
use std::path::Path;

pub struct TypeScriptParser;

impl LanguageParser for TypeScriptParser {
    fn language(&self) -> &str {
        "TypeScript"
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
        if parser.set_language(tree_sitter_typescript::language_tsx()).is_err() {
            result.syntax_ok = false;
            result.syntax_error = Some("Failed to load TypeScript grammar".to_string());
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

        let mut stack = vec![(tree.root_node(), false)]; // (node, in_class)

        while let Some((node, in_class)) = stack.pop() {
            let kind = node.kind();
            
            if kind == "class_declaration" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                        result.symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Class,
                            line: (name_node.start_position().row + 1) as u32,
                            column: name_node.start_position().column as u32,
                        });
                    }
                }
            } else if kind == "interface_declaration" || kind == "type_alias_declaration" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                        result.symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Interface,
                            line: (name_node.start_position().row + 1) as u32,
                            column: name_node.start_position().column as u32,
                        });
                    }
                }
            } else if kind == "function_declaration" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                        let sym_kind = if in_class { SymbolKind::Method } else { SymbolKind::Function };
                        result.symbols.push(Symbol {
                            name: name.to_string(),
                            kind: sym_kind,
                            line: (name_node.start_position().row + 1) as u32,
                            column: name_node.start_position().column as u32,
                        });
                    }
                }
            } else if kind == "method_definition" {
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
            } else if kind == "lexical_declaration" || kind == "variable_declaration" {
                for i in 0..node.child_count() {
                    if let Some(declarator) = node.child(i) {
                        if declarator.kind() == "variable_declarator" {
                            if let Some(value_node) = declarator.child_by_field_name("value") {
                                if value_node.kind() == "arrow_function" {
                                    if let Some(name_node) = declarator.child_by_field_name("name") {
                                        if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                                            result.symbols.push(Symbol {
                                                name: name.to_string(),
                                                kind: SymbolKind::Function,
                                                line: (name_node.start_position().row + 1) as u32,
                                                column: name_node.start_position().column as u32,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else if kind == "import_statement" {
                if let Some(source_node) = node.child_by_field_name("source") {
                    if let Ok(path_quoted) = source_node.utf8_text(content.as_bytes()) {
                        let path = path_quoted.trim_matches(&['\'', '"'][..]).to_string();
                        let is_rel = path.starts_with('.') || path.starts_with('/');
                        result.imports.push(Import {
                            path,
                            is_relative: is_rel,
                            line: (source_node.start_position().row + 1) as u32,
                            column: source_node.start_position().column as u32,
                        });
                    }
                }
            } else if kind == "call_expression" {
                if let Some(function_node) = node.child_by_field_name("function") {
                    if let Ok(func_name) = function_node.utf8_text(content.as_bytes()) {
                        if func_name == "require" || func_name == "import" {
                            if let Some(args_node) = node.child_by_field_name("arguments") {
                                for i in 0..args_node.child_count() {
                                    if let Some(arg) = args_node.child(i) {
                                        if arg.kind() == "string" {
                                            if let Ok(path_quoted) = arg.utf8_text(content.as_bytes()) {
                                                let path = path_quoted.trim_matches(&['\'', '"'][..]).to_string();
                                                let is_rel = path.starts_with('.') || path.starts_with('/');
                                                result.imports.push(Import {
                                                    path,
                                                    is_relative: is_rel,
                                                    line: (arg.start_position().row + 1) as u32,
                                                    column: arg.start_position().column as u32,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let next_in_class = kind == "class_declaration" || kind == "class_body" || (in_class && kind != "function_declaration" && kind != "method_definition");
            for i in (0..node.child_count()).rev() {
                if let Some(child) = node.child(i) {
                    stack.push((child, next_in_class));
                }
            }
        }

        result
    }
}
