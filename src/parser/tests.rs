#[cfg(test)]
mod parser_tests {
    use crate::parser::go::GoParser;
    use crate::parser::javascript::JavaScriptParser;
    use crate::parser::python::PythonParser;
    use crate::parser::rust::RustParser;
    use crate::parser::{LanguageParser, SymbolKind};

    #[test]
    fn test_python_parser_extracts_functions() {
        let content = r#"
def hello():
    pass

def world(x):
    return x
"#;
        let result = PythonParser.parse(std::path::Path::new("test.py"), content);
        assert!(result.syntax_ok);
        assert_eq!(result.symbols.len(), 2);
        assert_eq!(result.symbols[0].name, "hello");
        assert_eq!(result.symbols[0].kind, SymbolKind::Function);
        assert_eq!(result.symbols[0].line, 2);
        assert_eq!(result.symbols[1].name, "world");
        assert_eq!(result.symbols[1].line, 5);
    }

    #[test]
    fn test_python_parser_extracts_classes_and_methods() {
        let content = r#"
class MyClass:
    def method_one(self):
        pass

    def method_two(self):
        pass
"#;
        let result = PythonParser.parse(std::path::Path::new("test.py"), content);
        assert!(result.syntax_ok);
        assert_eq!(result.symbols.len(), 3);
        assert_eq!(result.symbols[0].name, "MyClass");
        assert_eq!(result.symbols[0].kind, SymbolKind::Class);
        assert_eq!(result.symbols[0].line, 2);
        assert_eq!(result.symbols[1].name, "method_one");
        assert_eq!(result.symbols[1].kind, SymbolKind::Method);
        assert_eq!(result.symbols[1].line, 3);
    }

    #[test]
    fn test_python_parser_extracts_imports() {
        let content = r#"
import os
from collections import defaultdict
from .utils import helper
"#;
        let result = PythonParser.parse(std::path::Path::new("test.py"), content);
        assert!(result.syntax_ok);
        assert_eq!(result.imports.len(), 3);
        assert!(result.imports.iter().any(|i| i.path == "os"));
        assert!(result.imports.iter().any(|i| i.path == "collections"));
        assert!(result.imports.iter().any(|i| i.path == ".utils"));
    }

    #[test]
    fn test_python_parser_import_line_numbers() {
        let content = "import os\nimport sys\nfrom .local import helper\n";
        let result = PythonParser.parse(std::path::Path::new("test.py"), content);
        assert_eq!(result.imports[0].line, 1);
        assert_eq!(result.imports[0].path, "os");
        assert!(!result.imports[0].is_relative);
        assert_eq!(result.imports[1].line, 2);
        assert_eq!(result.imports[1].path, "sys");
        assert_eq!(result.imports[2].line, 3);
        assert_eq!(result.imports[2].path, ".local");
        assert!(result.imports[2].is_relative);
    }

    #[test]
    fn test_javascript_parser_extracts_functions() {
        let content = r#"
function hello() {}
const world = async () => {}
"#;
        let result = JavaScriptParser.parse(std::path::Path::new("test.js"), content);
        assert!(result.syntax_ok);
        assert!(result.symbols.len() >= 2);
    }

    #[test]
    fn test_javascript_parser_extracts_classes() {
        let content = r#"
class MyClass {
    methodOne() {}
    async methodTwo() {}
}
"#;
        let result = JavaScriptParser.parse(std::path::Path::new("test.js"), content);
        assert!(result.syntax_ok);
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "MyClass" && matches!(s.kind, SymbolKind::Class)));
    }

    #[test]
    fn test_javascript_parser_extracts_imports() {
        let content = r#"
import React from 'react';
import { useState } from 'react';
const fs = require('fs');
import('./dynamic');
"#;
        let result = JavaScriptParser.parse(std::path::Path::new("test.js"), content);
        assert!(result.syntax_ok);
        assert_eq!(result.imports.len(), 4);
    }

    #[test]
    fn test_rust_parser_extracts_functions() {
        let content = r#"
fn hello() {}
fn world(x: i32) -> i32 { x }
"#;
        let result = RustParser.parse(std::path::Path::new("test.rs"), content);
        assert!(result.syntax_ok);
        assert!(result.symbols.iter().any(|s| s.name == "hello"));
        assert!(result.symbols.iter().any(|s| s.name == "world"));
    }

    #[test]
    fn test_rust_parser_extracts_structs_and_imports() {
        let content = r#"
use std::collections::HashMap;
use crate::utils::helper;

struct MyStruct {
    field: i32,
}
"#;
        let result = RustParser.parse(std::path::Path::new("test.rs"), content);
        assert!(result.syntax_ok);
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "MyStruct" && matches!(s.kind, SymbolKind::Struct)));
        assert!(result
            .imports
            .iter()
            .any(|i| i.path.contains("std::collections")));
    }

    #[test]
    fn test_go_parser_extracts_functions() {
        let content = r#"
package main

func hello() {}
func (r *Receiver) method() {}
"#;
        let result = GoParser.parse(std::path::Path::new("test.go"), content);
        assert!(result.syntax_ok);
        assert!(result.symbols.iter().any(|s| s.name == "hello"));
        assert!(result.symbols.iter().any(|s| s.name == "method"));
    }

    #[test]
    fn test_go_parser_extracts_imports() {
        let content = r#"
import "fmt"
import (
    "os"
    "strings"
)
"#;
        let result = GoParser.parse(std::path::Path::new("test.go"), content);
        assert!(result.syntax_ok);
        assert!(result.imports.iter().any(|i| i.path == "fmt"));
        assert!(result.imports.iter().any(|i| i.path == "os"));
        assert!(result.imports.iter().any(|i| i.path == "strings"));
    }

    // --- Phase 3: Line number and import classification tests ---

    #[test]
    fn test_python_parser_line_numbers() {
        let content = "def foo():\n    pass\ndef bar():\n    pass\n";
        let result = PythonParser.parse(std::path::Path::new("test.py"), content);
        assert_eq!(result.symbols.len(), 2);
        assert_eq!(result.symbols[0].name, "foo");
        assert_eq!(result.symbols[0].line, 1);
        assert_eq!(result.symbols[1].name, "bar");
        assert_eq!(result.symbols[1].line, 3);
    }

    #[test]
    fn test_python_parser_relative_imports() {
        let content = "from .utils import helper\nfrom ..parent import func\nfrom os import path\n";
        let result = PythonParser.parse(std::path::Path::new("test.py"), content);
        assert_eq!(result.imports.len(), 3);
        assert_eq!(result.imports[0].path, ".utils");
        assert!(result.imports[0].is_relative);
        assert_eq!(result.imports[1].path, "..parent");
        assert!(result.imports[1].is_relative);
        assert_eq!(result.imports[2].path, "os");
        assert!(!result.imports[2].is_relative);
    }

    #[test]
    fn test_javascript_parser_line_numbers() {
        let content = "function first() {}\nconst second = () => {}\n";
        let result = JavaScriptParser.parse(std::path::Path::new("test.js"), content);
        assert!(result.symbols.iter().any(|s| s.name == "first" && s.line == 1));
        assert!(result.symbols.iter().any(|s| s.name == "second" && s.line == 2));
    }

    #[test]
    fn test_javascript_parser_relative_imports() {
        let content = "import { foo } from './local';\nimport { bar } from '/absolute';\nimport { baz } from 'package';\n";
        let result = JavaScriptParser.parse(std::path::Path::new("test.js"), content);
        assert_eq!(result.imports.len(), 3);
        assert_eq!(result.imports[0].path, "./local");
        assert!(result.imports[0].is_relative);
        assert_eq!(result.imports[1].path, "/absolute");
        assert!(result.imports[1].is_relative);
        assert_eq!(result.imports[2].path, "package");
        assert!(!result.imports[2].is_relative);
    }

    #[test]
    fn test_javascript_parser_import_line_numbers() {
        let content = "import a from 'a';\nconst b = require('b');\nimport('c');\n";
        let result = JavaScriptParser.parse(std::path::Path::new("test.js"), content);
        assert_eq!(result.imports[0].line, 1);
        assert_eq!(result.imports[0].path, "a");
        assert_eq!(result.imports[1].line, 2);
        assert_eq!(result.imports[1].path, "b");
        assert_eq!(result.imports[2].line, 3);
        assert_eq!(result.imports[2].path, "c");
    }

    #[test]
    fn test_rust_parser_line_numbers() {
        let content = "fn first() {}\nfn second() {}\n";
        let result = RustParser.parse(std::path::Path::new("test.rs"), content);
        assert_eq!(result.symbols.len(), 2);
        assert_eq!(result.symbols[0].name, "first");
        assert_eq!(result.symbols[0].line, 1);
        assert_eq!(result.symbols[1].name, "second");
        assert_eq!(result.symbols[1].line, 2);
    }

    #[test]
    fn test_rust_parser_relative_imports() {
        let content = "use crate::utils::helper;\nuse super::module;\nuse self::item;\nuse std::collections::HashMap;\n";
        let result = RustParser.parse(std::path::Path::new("test.rs"), content);
        assert_eq!(result.imports.len(), 4);
        assert!(result.imports[0].is_relative);
        assert!(result.imports[1].is_relative);
        assert!(result.imports[2].is_relative);
        assert!(!result.imports[3].is_relative);
    }

    #[test]
    fn test_rust_parser_struct_and_trait_line_numbers() {
        let content = "struct Foo;\nenum Bar { A, B }\ntrait Baz {}\n";
        let result = RustParser.parse(std::path::Path::new("test.rs"), content);
        assert!(result.symbols.iter().any(|s| s.name == "Foo" && s.line == 1));
        assert!(result.symbols.iter().any(|s| s.name == "Bar" && s.line == 2));
        assert!(result.symbols.iter().any(|s| s.name == "Baz" && s.line == 3));
    }

    #[test]
    fn test_go_parser_line_numbers() {
        let content = "func main() {}\nfunc helper() {}\n";
        let result = GoParser.parse(std::path::Path::new("test.go"), content);
        assert!(result.symbols.iter().any(|s| s.name == "main" && s.line == 1));
        assert!(result.symbols.iter().any(|s| s.name == "helper" && s.line == 2));
    }

    #[test]
    fn test_go_parser_relative_imports() {
        let content = "import \"fmt\"\nimport (\n    \"./local\"\n    \"strings\"\n)\n";
        let result = GoParser.parse(std::path::Path::new("test.go"), content);
        assert_eq!(result.imports.len(), 3);
        let fmt_imp = result.imports.iter().find(|i| i.path == "fmt").unwrap();
        assert!(!fmt_imp.is_relative);
        let local_imp = result.imports.iter().find(|i| i.path == "./local").unwrap();
        assert!(local_imp.is_relative);
        assert_eq!(local_imp.line, 3);
        let strings_imp = result.imports.iter().find(|i| i.path == "strings").unwrap();
        assert!(!strings_imp.is_relative);
        assert_eq!(strings_imp.line, 4);
    }

    #[test]
    fn test_go_parser_struct_and_interface_line_numbers() {
        let content = "type MyType struct {\n    field int\n}\ntype MyInterface interface {\n    Method()\n}\n";
        let result = GoParser.parse(std::path::Path::new("test.go"), content);
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "MyType" && matches!(s.kind, SymbolKind::Struct) && s.line == 1));
        assert!(result
            .symbols
            .iter()
            .any(|s| s.name == "MyInterface" && matches!(s.kind, SymbolKind::Interface) && s.line == 4));
    }

    #[test]
    fn test_go_parser_method_extraction() {
        let content = "func (r *Receiver) DoThing() {}\nfunc standalone() {}\n";
        let result = GoParser.parse(std::path::Path::new("test.go"), content);
        assert!(result.symbols.iter().any(|s| s.name == "DoThing" && matches!(s.kind, SymbolKind::Method)));
        assert!(result.symbols.iter().any(|s| s.name == "standalone" && matches!(s.kind, SymbolKind::Function)));
    }

    #[test]
    fn test_rust_parser_extraction_with_line_numbers() {
        let content = "use std::io;
fn greet() {}
struct Config;
enum Mode { A, B }
trait Debug {}
";
        let result = RustParser.parse(std::path::Path::new("test.rs"), content);
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].path, "std::io");
        assert_eq!(result.imports[0].line, 1);
        assert!(!result.imports[0].is_relative);
        assert_eq!(result.symbols.len(), 4);
        assert_eq!(result.symbols[0].name, "greet");
        assert_eq!(result.symbols[0].line, 2);
        assert_eq!(result.symbols[1].name, "Config");
        assert_eq!(result.symbols[1].kind, SymbolKind::Struct);
        assert_eq!(result.symbols[1].line, 3);
        assert_eq!(result.symbols[2].name, "Mode");
        assert_eq!(result.symbols[2].kind, SymbolKind::Enum);
        assert_eq!(result.symbols[2].line, 4);
        assert_eq!(result.symbols[3].name, "Debug");
        assert_eq!(result.symbols[3].kind, SymbolKind::Trait);
        assert_eq!(result.symbols[3].line, 5);
    }

    #[test]
    fn test_tree_sitter_ignores_comments_and_strings() {
        let content = r#"
// fn fake_function() {}
let x = "fn fake_string_function() {}";
fn real_function() {}
"#;
        let result = RustParser.parse(std::path::Path::new("test.rs"), content);
        assert!(result.syntax_ok);
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].name, "real_function");
    }

    #[test]
    fn test_tree_sitter_nested_declarations() {
        let content = r#"
fn outer() {
    fn inner() {}
}
"#;
        let result = RustParser.parse(std::path::Path::new("test.rs"), content);
        assert!(result.syntax_ok);
        assert_eq!(result.symbols.len(), 2);
        assert!(result.symbols.iter().any(|s| s.name == "outer"));
        assert!(result.symbols.iter().any(|s| s.name == "inner"));
    }
}

