use crate::parser::{Import, LanguageParser, ParseResult, Symbol, SymbolKind};
use std::path::Path;

pub struct PythonParser;

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
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();

            // Track class context
            if trimmed.starts_with("class ") {
                in_class = true;
                class_indent = indent;
                if let Some(name) = trimmed[6..].split_whitespace().next() {
                    let name = name.trim_end_matches(['(', ':']).to_string();
                    result.symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Class,
                        line: line_num as u32 + 1,
                        column: 0,
                    });
                }
                continue;
            }

            // Exit class context when we see a line with same or less indentation
            if in_class && indent <= class_indent && !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("def ") && !trimmed.starts_with("    ") {
                in_class = false;
            }

            // Extract methods
            if trimmed.starts_with("def ") {
                let rest = &trimmed[4..];
                if let Some(name) = rest.split('(').next() {
                    let name = name.trim().to_string();
                    if in_class {
                        result.symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Method,
                            line: line_num as u32 + 1,
                            column: 0,
                        });
                    } else {
                        result.symbols.push(Symbol {
                            name,
                            kind: SymbolKind::Function,
                            line: line_num as u32 + 1,
                            column: 0,
                        });
                    }
                }
            }

            // Extract imports
            if trimmed.starts_with("import ") {
                let name = trimmed[7..].split_whitespace().next().unwrap_or("").to_string();
                result.imports.push(Import {
                    path: name,
                    is_relative: false,
                    line: line_num as u32 + 1,
                    column: 0,
                });
            }

            if trimmed.starts_with("from ") {
                let parts: Vec<&str> = trimmed[5..].split_whitespace().collect();
                if parts.len() >= 3 && parts[1] == "import" {
                    let module = parts[0].to_string();
                    let is_rel = module.starts_with('.');
                    result.imports.push(Import {
                        path: module,
                        is_relative: is_rel,
                        line: line_num as u32 + 1,
                        column: 0,
                    });
                }
            }
        }

        result
    }
}
