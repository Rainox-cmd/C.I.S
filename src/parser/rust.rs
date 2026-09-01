use crate::parser::{Import, LanguageParser, ParseResult, Symbol, SymbolKind};
use std::path::Path;

pub struct RustParser;

impl LanguageParser for RustParser {
    fn language(&self) -> &str {
        "Rust"
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
        if parser.set_language(tree_sitter_rust::language()).is_err() {
            result.syntax_ok = false;
            result.syntax_error = Some("Failed to load Rust grammar".to_string());
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

            let sym_kind = match kind {
                "function_item" => Some(SymbolKind::Function),
                "struct_item" => Some(SymbolKind::Struct),
                "enum_item" => Some(SymbolKind::Enum),
                "trait_item" => Some(SymbolKind::Trait),
                _ => None,
            };

            if let Some(sk) = sym_kind {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                        result.symbols.push(Symbol {
                            name: name.to_string(),
                            kind: sk,
                            line: (name_node.start_position().row + 1) as u32,
                            column: name_node.start_position().column as u32,
                        });
                    }
                }
            } else if kind == "use_declaration" {
                if let Some(arg_node) = node.child_by_field_name("argument") {
                    if let Ok(path_text) = arg_node.utf8_text(content.as_bytes()) {
                        let path = path_text.to_string();
                        let is_rel = path.starts_with("crate::")
                            || path.starts_with("super::")
                            || path.starts_with("self::");
                        
                        result.imports.push(Import {
                            path,
                            is_relative: is_rel,
                            line: (arg_node.start_position().row + 1) as u32,
                            column: arg_node.start_position().column as u32,
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
