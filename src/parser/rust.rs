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

        let re_function = regex::Regex::new(r"fn\s+(\w+)\s*[<(]").unwrap();
        let re_struct = regex::Regex::new(r"struct\s+(\w+)\s*[;{<]").unwrap();
        let re_enum = regex::Regex::new(r"enum\s+(\w+)\s*\{").unwrap();
        let re_trait = regex::Regex::new(r"trait\s+(\w+)\s*[{<]").unwrap();
        let re_use = regex::Regex::new(r"use\s+(.+?);").unwrap();

        for (line_num, line) in content.lines().enumerate() {
            let line_no = line_num + 1;
            let trimmed = line.trim_start();

            if let Some(cap) = re_function.captures(trimmed) {
                if let Some(name) = cap.get(1) {
                    result.symbols.push(Symbol {
                        name: name.as_str().to_string(),
                        kind: SymbolKind::Function,
                        line: line_no as u32,
                        column: cap.get(0).unwrap().start() as u32,
                    });
                }
            }

            if let Some(cap) = re_struct.captures(trimmed) {
                if let Some(name) = cap.get(1) {
                    result.symbols.push(Symbol {
                        name: name.as_str().to_string(),
                        kind: SymbolKind::Struct,
                        line: line_no as u32,
                        column: cap.get(0).unwrap().start() as u32,
                    });
                }
            }

            if let Some(cap) = re_enum.captures(trimmed) {
                if let Some(name) = cap.get(1) {
                    result.symbols.push(Symbol {
                        name: name.as_str().to_string(),
                        kind: SymbolKind::Enum,
                        line: line_no as u32,
                        column: cap.get(0).unwrap().start() as u32,
                    });
                }
            }

            if let Some(cap) = re_trait.captures(trimmed) {
                if let Some(name) = cap.get(1) {
                    result.symbols.push(Symbol {
                        name: name.as_str().to_string(),
                        kind: SymbolKind::Trait,
                        line: line_no as u32,
                        column: cap.get(0).unwrap().start() as u32,
                    });
                }
            }

            if let Some(cap) = re_use.captures(trimmed) {
                if let Some(path_match) = cap.get(1) {
                    let path = path_match.as_str().trim_end_matches(';').to_string();
                    let is_rel = path.starts_with("crate::")
                        || path.starts_with("super::")
                        || path.starts_with("self::");
                    result.imports.push(Import {
                        path,
                        is_relative: is_rel,
                        line: line_no as u32,
                        column: cap.get(0).unwrap().start() as u32,
                    });
                }
            }
        }

        result
    }
}
