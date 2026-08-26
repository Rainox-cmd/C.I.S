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

        let re_function = regex::Regex::new(r"function\s+(\w+)\s*\(").unwrap();
        let re_class = regex::Regex::new(r"class\s+(\w+)\s*[{(]").unwrap();
        let re_arrow =
            regex::Regex::new(r"(?:const|let|var)\s+(\w+)\s*=\s*(?:async\s+)?(?:\([^)]*\)|[^=])\s*=>")
                .unwrap();
        let re_es6_import =
            regex::Regex::new(r#"import\s+.*?from\s+['"]([^'"]+)['"]"#).unwrap();
        let re_require =
            regex::Regex::new(r#"require\s*\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap();
        let re_dynamic_import =
            regex::Regex::new(r#"import\s*\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap();
        let re_export_name =
            regex::Regex::new(r"(?:export\s+)?(?:function|const|let|var|class)\s+(\w+)").unwrap();

        let mut in_class = false;
        let mut class_indent = 0;

        for (line_num, line) in content.lines().enumerate() {
            let line_no = line_num + 1;
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();

            if in_class
                && !trimmed.is_empty()
                && !trimmed.starts_with("class ")
                && !trimmed.starts_with("//")
                && !trimmed.starts_with("/*")
                && !trimmed.starts_with("*")
                && indent <= class_indent
            {
                in_class = false;
            }

            if let Some(cap) = re_class.captures(trimmed) {
                if let Some(name) = cap.get(1) {
                    in_class = true;
                    class_indent = indent;
                    result.symbols.push(Symbol {
                        name: name.as_str().to_string(),
                        kind: SymbolKind::Class,
                        line: line_no as u32,
                        column: cap.get(0).unwrap().start() as u32,
                    });
                }
            }

            if let Some(cap) = re_function.captures(trimmed) {
                if let Some(name) = cap.get(1) {
                    let kind = if in_class {
                        SymbolKind::Method
                    } else {
                        SymbolKind::Function
                    };
                    result.symbols.push(Symbol {
                        name: name.as_str().to_string(),
                        kind,
                        line: line_no as u32,
                        column: cap.get(0).unwrap().start() as u32,
                    });
                }
            }

            if let Some(cap) = re_arrow.captures(trimmed) {
                if let Some(name) = cap.get(1) {
                    result.symbols.push(Symbol {
                        name: name.as_str().to_string(),
                        kind: SymbolKind::Function,
                        line: line_no as u32,
                        column: cap.get(0).unwrap().start() as u32,
                    });
                }
            }

            if let Some(cap) = re_es6_import.captures(line) {
                if let Some(path_match) = cap.get(1) {
                    let path = path_match.as_str().to_string();
                    let is_rel = path.starts_with('.') || path.starts_with('/');
                    result.imports.push(Import {
                        path,
                        is_relative: is_rel,
                        line: line_no as u32,
                        column: cap.get(0).unwrap().start() as u32,
                    });
                }
            }

            if let Some(cap) = re_require.captures(line) {
                if let Some(path_match) = cap.get(1) {
                    let path = path_match.as_str().to_string();
                    let is_rel = path.starts_with('.') || path.starts_with('/');
                    result.imports.push(Import {
                        path,
                        is_relative: is_rel,
                        line: line_no as u32,
                        column: cap.get(0).unwrap().start() as u32,
                    });
                }
            }

            if let Some(cap) = re_dynamic_import.captures(line) {
                if let Some(path_match) = cap.get(1) {
                    let path = path_match.as_str().to_string();
                    let is_rel = path.starts_with('.') || path.starts_with('/');
                    result.imports.push(Import {
                        path,
                        is_relative: is_rel,
                        line: line_no as u32,
                        column: cap.get(0).unwrap().start() as u32,
                    });
                }
            }

            if let Some(cap) = re_export_name.captures(trimmed) {
                if let Some(name) = cap.get(1) {
                    let line_str = trimmed;
                    if line_str.starts_with("export ") {
                        if let Some(_) = line_str.strip_prefix("export ") {
                            result.exports.push(crate::parser::Export {
                                name: name.as_str().to_string(),
                                kind: if line_str.contains("function") {
                                    SymbolKind::Function
                                } else if line_str.contains("class") {
                                    SymbolKind::Class
                                } else {
                                    SymbolKind::Variable
                                },
                                line: line_no as u32,
                            });
                        }
                    }
                }
            }
        }

        result
    }
}
