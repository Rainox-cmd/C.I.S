use crate::parser::{Import, LanguageParser, ParseResult, Symbol, SymbolKind};
use std::path::Path;

pub struct JavaScriptParser;

impl LanguageParser for JavaScriptParser {
    fn language(&self) -> &str {
        "JavaScript"
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
        for cap in regex::Regex::new(r"function\s+(\w+)\s*\(")
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

        // Extract arrow functions and const/let/var assignments
        for cap in regex::Regex::new(
            r"(?:const|let|var)\s+(\w+)\s*=\s*(?:async\s+)?(?:\([^)]*\)|[^=])\s*=>",
        )
        .unwrap()
        .find_iter(content)
        {
            let name = cap
                .as_str()
                .split_whitespace()
                .nth(1)
                .unwrap()
                .trim_end_matches('=')
                .trim()
                .to_string();
            result.symbols.push(Symbol {
                name,
                kind: SymbolKind::Function,
                line: 0,
                column: 0,
            });
        }

        // Extract classes
        for cap in regex::Regex::new(r"class\s+(\w+)\s*[{(]")
            .unwrap()
            .find_iter(content)
        {
            let name = cap
                .as_str()
                .split_whitespace()
                .nth(1)
                .unwrap()
                .trim_end_matches(['{', '('])
                .to_string();
            result.symbols.push(Symbol {
                name,
                kind: SymbolKind::Class,
                line: 0,
                column: 0,
            });
        }

        // Extract methods inside classes
        for cap in regex::Regex::new(r"class\s+(\w+)[\s\S]*?(?:async\s+)?(\w+)\s*\([^)]*\)\s*\{")
            .unwrap()
            .find_iter(content)
        {
            let method_name = cap
                .as_str()
                .split_whitespace()
                .last()
                .unwrap()
                .trim_end_matches('(')
                .to_string();
            result.symbols.push(Symbol {
                name: method_name,
                kind: SymbolKind::Method,
                line: 0,
                column: 0,
            });
        }

        // Extract imports
        for cap in regex::Regex::new(r#"import\s+.*?from\s+['"]([^'"]+)['"]"#)
            .unwrap()
            .find_iter(content)
        {
            let path = cap
                .as_str()
                .split('\'')
                .nth(1)
                .or_else(|| cap.as_str().split('"').nth(1))
                .unwrap_or("")
                .to_string();
            let is_rel = path.starts_with('.') || path.starts_with('/');
            result.imports.push(Import {
                path,
                is_relative: is_rel,
                line: 0,
                column: 0,
            });
        }

        for cap in regex::Regex::new(r#"require\s*\(\s*['"]([^'"]+)['"]\s*\)"#)
            .unwrap()
            .find_iter(content)
        {
            let path = cap
                .as_str()
                .split('\'')
                .nth(1)
                .or_else(|| cap.as_str().split('"').nth(1))
                .unwrap_or("")
                .to_string();
            let is_rel = path.starts_with('.') || path.starts_with('/');
            result.imports.push(Import {
                path,
                is_relative: is_rel,
                line: 0,
                column: 0,
            });
        }

        for cap in regex::Regex::new(r#"import\s*\(\s*['"]([^'"]+)['"]\s*\)"#)
            .unwrap()
            .find_iter(content)
        {
            let path = cap
                .as_str()
                .split('\'')
                .nth(1)
                .or_else(|| cap.as_str().split('"').nth(1))
                .unwrap_or("")
                .to_string();
            let is_rel = path.starts_with('.') || path.starts_with('/');
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
