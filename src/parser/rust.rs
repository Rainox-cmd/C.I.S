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

        // Extract functions
        for cap in regex::Regex::new(r"fn\s+(\w+)\s*[<(]").unwrap().find_iter(content) {
            let name = cap.as_str().split_whitespace().nth(1).unwrap().trim_end_matches(['<', '(']).to_string();
            result.symbols.push(Symbol {
                name,
                kind: SymbolKind::Function,
                line: 0,
                column: 0,
            });
        }

        // Extract structs
        for cap in regex::Regex::new(r"struct\s+(\w+)\s*[;{<]").unwrap().find_iter(content) {
            let name = cap.as_str().split_whitespace().nth(1).unwrap().trim_end_matches([';', '{', '<']).to_string();
            result.symbols.push(Symbol {
                name,
                kind: SymbolKind::Struct,
                line: 0,
                column: 0,
            });
        }

        // Extract enums
        for cap in regex::Regex::new(r"enum\s+(\w+)\s*\{").unwrap().find_iter(content) {
            let name = cap.as_str().split_whitespace().nth(1).unwrap().trim_end_matches('{').to_string();
            result.symbols.push(Symbol {
                name,
                kind: SymbolKind::Enum,
                line: 0,
                column: 0,
            });
        }

        // Extract traits
        for cap in regex::Regex::new(r"trait\s+(\w+)\s*[{<]").unwrap().find_iter(content) {
            let name = cap.as_str().split_whitespace().nth(1).unwrap().trim_end_matches(['{', '<']).to_string();
            result.symbols.push(Symbol {
                name,
                kind: SymbolKind::Trait,
                line: 0,
                column: 0,
            });
        }

        // Extract imports (use statements)
        for cap in regex::Regex::new(r"use\s+(.+?);").unwrap().find_iter(content) {
            let path = cap.as_str().split_whitespace().nth(1).unwrap().trim_end_matches(';').to_string();
            let is_rel = path.starts_with("crate::") || path.starts_with("super::") || path.starts_with("self::");
            result.imports.push(Import {
                path,
                is_relative: is_rel,
                line: 0,
                column: 0,
            });
        }

        result
    }
}
