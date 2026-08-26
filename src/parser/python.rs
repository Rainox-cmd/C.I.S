use crate::parser::{Import, LanguageParser, ParseResult, Symbol, SymbolKind};
use std::path::Path;

pub struct PythonParser;

fn trim_suffix_chars(s: &str) -> &str {
    s.trim_end_matches(|c| c == '(' || c == ':')
}

impl LanguageParser for PythonParser {
    fn language(&self) -> &str {
        "Python"
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

        let mut in_class = false;
        let mut class_indent = 0;

        for (line_num, line) in content.lines().enumerate() {
            let line_no = line_num + 1;
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();

            if let Some(rest) = trimmed.strip_prefix("class ") {
                in_class = true;
                class_indent = indent;
                let name = trim_suffix_chars(
                    rest.split_whitespace().next().unwrap_or(""),
                );
                if !name.is_empty() {
                    result.symbols.push(Symbol {
                        name: name.to_string(),
                        kind: SymbolKind::Class,
                        line: line_no as u32,
                        column: indent as u32,
                    });
                }
                continue;
            }

            if in_class
                && indent <= class_indent
                && !trimmed.is_empty()
                && !trimmed.starts_with('#')
                && !trimmed.starts_with("def ")
                && !trimmed.starts_with("    ")
            {
                in_class = false;
            }

            if let Some(rest) = trimmed.strip_prefix("def ") {
                let name = rest.split('(').next().unwrap_or("").trim().to_string();
                if !name.is_empty() {
                    let kind = if in_class {
                        SymbolKind::Method
                    } else {
                        SymbolKind::Function
                    };
                    result.symbols.push(Symbol {
                        name,
                        kind,
                        line: line_no as u32,
                        column: indent as u32,
                    });
                }
            }

            if let Some(rest) = trimmed.strip_prefix("import ") {
                let module = rest.split_whitespace().next().unwrap_or("").to_string();
                if !module.is_empty() {
                    result.imports.push(Import {
                        path: module,
                        is_relative: false,
                        line: line_no as u32,
                        column: indent as u32,
                    });
                }
            }

            if let Some(rest) = trimmed.strip_prefix("from ") {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if parts.len() >= 3 && parts[1] == "import" {
                    let module = parts[0].to_string();
                    let is_rel = module.starts_with('.');
                    result.imports.push(Import {
                        path: module,
                        is_relative: is_rel,
                        line: line_no as u32,
                        column: indent as u32,
                    });
                }
            }
        }

        result
    }
}
