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

        // Extract functions
        for cap in regex::Regex::new(r"func\s+(\w+)\s*\(")
            .unwrap()
            .find_iter(content)
        {
            let name = cap
                .as_str()
                .split_whitespace()
                .nth(1)
                .unwrap()
                .trim_end_matches('(')
                .to_string();
            result.symbols.push(Symbol {
                name,
                kind: SymbolKind::Function,
                line: 0,
                column: 0,
            });
        }

        // Extract methods (func with receiver)
        for cap in regex::Regex::new(r"func\s*\([^)]+\)\s*(\w+)\s*\(")
            .unwrap()
            .find_iter(content)
        {
            let name = cap
                .as_str()
                .split_whitespace()
                .last()
                .unwrap()
                .trim_end_matches('(')
                .to_string();
            result.symbols.push(Symbol {
                name,
                kind: SymbolKind::Method,
                line: 0,
                column: 0,
            });
        }

        // Extract structs
        for cap in regex::Regex::new(r"type\s+(\w+)\s+struct\s*\{")
            .unwrap()
            .find_iter(content)
        {
            let name = cap.as_str().split_whitespace().nth(1).unwrap().to_string();
            result.symbols.push(Symbol {
                name,
                kind: SymbolKind::Struct,
                line: 0,
                column: 0,
            });
        }

        // Extract interfaces
        for cap in regex::Regex::new(r"type\s+(\w+)\s+interface\s*\{")
            .unwrap()
            .find_iter(content)
        {
            let name = cap.as_str().split_whitespace().nth(1).unwrap().to_string();
            result.symbols.push(Symbol {
                name,
                kind: SymbolKind::Interface,
                line: 0,
                column: 0,
            });
        }

        // Extract imports
        for cap in regex::Regex::new(r"import\s*\(")
            .unwrap()
            .find_iter(content)
        {
            let start = cap.end();
            if let Some(end) = content[start..].find(")") {
                let block = &content[start..start + end];
                for line in block.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with("//") {
                        continue;
                    }
                    if let Some(quote_start) = line.find('"') {
                        if let Some(quote_end) = line[quote_start + 1..].find('"') {
                            let path =
                                line[quote_start + 1..quote_start + 1 + quote_end].to_string();
                            let is_rel = path.starts_with(".");
                            result.imports.push(Import {
                                path,
                                is_relative: is_rel,
                                line: 0,
                                column: 0,
                            });
                        }
                    }
                }
            }
        }

        for cap in regex::Regex::new(r#"import\s+"([^"]+)"#)
            .unwrap()
            .find_iter(content)
        {
            let path = cap.as_str().split('"').nth(1).unwrap_or("").to_string();
            if !path.is_empty() {
                let is_rel = path.starts_with(".");
                result.imports.push(Import {
                    path,
                    is_relative: is_rel,
                    line: 0,
                    column: 0,
                });
            }
        }

        result
    }
}
