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

        let re_func = regex::Regex::new(r"func\s+(\w+)\s*\(").unwrap();
        let re_method = regex::Regex::new(r"func\s*\([^)]+\)\s*(\w+)\s*\(").unwrap();
        let re_struct = regex::Regex::new(r"type\s+(\w+)\s+struct\s*\{").unwrap();
        let re_interface = regex::Regex::new(r"type\s+(\w+)\s+interface\s*\{").unwrap();
        let re_import_block = regex::Regex::new(r"import\s*\(").unwrap();
        let re_import_single = regex::Regex::new(r#"import\s+"([^"]+)"#).unwrap();

        let lines: Vec<&str> = content.lines().collect();
        let mut in_import_block = false;
        let mut in_type_block = false;
        let mut type_block_indent = 0;

        for (line_num, line) in lines.iter().enumerate() {
            let line_no = line_num + 1;
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();

            if in_import_block {
                if trimmed == ")" {
                    in_import_block = false;
                    continue;
                }
                if let Some(quote_start) = trimmed.find('"') {
                    if let Some(quote_end) = trimmed[quote_start + 1..].find('"') {
                        let path = trimmed[quote_start + 1..quote_start + 1 + quote_end].to_string();
                        let is_rel = path.starts_with(".");
                        result.imports.push(Import {
                            path,
                            is_relative: is_rel,
                            line: line_no as u32,
                            column: quote_start as u32,
                        });
                    }
                }
                continue;
            }

            if in_type_block && indent <= type_block_indent && !trimmed.is_empty() && !trimmed.starts_with("//") {
                in_type_block = false;
            }

            if let Some(cap) = re_func.captures(trimmed) {
                if let Some(name) = cap.get(1) {
                    result.symbols.push(Symbol {
                        name: name.as_str().to_string(),
                        kind: SymbolKind::Function,
                        line: line_no as u32,
                        column: cap.get(0).unwrap().start() as u32,
                    });
                }
            }

            if let Some(cap) = re_method.captures(trimmed) {
                if let Some(name) = cap.get(1) {
                    result.symbols.push(Symbol {
                        name: name.as_str().to_string(),
                        kind: SymbolKind::Method,
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
                    in_type_block = true;
                    type_block_indent = indent;
                }
            }

            if let Some(cap) = re_interface.captures(trimmed) {
                if let Some(name) = cap.get(1) {
                    result.symbols.push(Symbol {
                        name: name.as_str().to_string(),
                        kind: SymbolKind::Interface,
                        line: line_no as u32,
                        column: cap.get(0).unwrap().start() as u32,
                    });
                    in_type_block = true;
                    type_block_indent = indent;
                }
            }

            if re_import_block.is_match(trimmed) {
                in_import_block = true;
                continue;
            }

            if let Some(cap) = re_import_single.captures(trimmed) {
                if let Some(path_match) = cap.get(1) {
                    let path = path_match.as_str().to_string();
                    let is_rel = path.starts_with(".");
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
