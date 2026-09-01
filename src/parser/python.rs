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

        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(tree_sitter_python::language()).is_err() {
            result.syntax_ok = false;
            result.syntax_error = Some("Failed to load Python grammar".to_string());
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
            
            if kind == "class_definition" {
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
            } else if kind == "function_definition" {
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
            } else if kind == "import_statement" {
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "dotted_name" {
                            if let Ok(name) = child.utf8_text(content.as_bytes()) {
                                result.imports.push(Import {
                                    path: name.to_string(),
                                    is_relative: false,
                                    line: (child.start_position().row + 1) as u32,
                                    column: child.start_position().column as u32,
                                });
                            }
                        } else if child.kind() == "aliased_import" {
                             if let Some(name_node) = child.child_by_field_name("name") {
                                if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                                    result.imports.push(Import {
                                        path: name.to_string(),
                                        is_relative: false,
                                        line: (name_node.start_position().row + 1) as u32,
                                        column: name_node.start_position().column as u32,
                                    });
                                }
                            }
                        }
                    }
                }
            } else if kind == "import_from_statement" {
                let mut path = String::new();
                let mut is_relative = false;
                
                let mut relative_dots = 0;
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "." {
                            relative_dots += 1;
                            is_relative = true;
                        }
                    }
                }
                
                let module_name_node = node.child_by_field_name("module_name");
                if let Some(mn) = module_name_node {
                    if let Ok(mn_text) = mn.utf8_text(content.as_bytes()) {
                        path = mn_text.to_string();
                    }
                }
                
                let mut final_path = String::new();
                for _ in 0..relative_dots {
                    final_path.push('.');
                }
                final_path.push_str(&path);
                
                if !final_path.is_empty() {
                    result.imports.push(Import {
                        path: final_path,
                        is_relative,
                        line: (node.start_position().row + 1) as u32,
                        column: node.start_position().column as u32,
                    });
                }
            }

            let next_in_class = kind == "class_definition" || (in_class && kind != "function_definition");
            for i in (0..node.child_count()).rev() {
                if let Some(child) = node.child(i) {
                    stack.push((child, next_in_class));
                }
            }
        }

        result
    }
}
