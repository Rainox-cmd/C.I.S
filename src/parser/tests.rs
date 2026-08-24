#[cfg(test)]
mod tests {
    use crate::parser::{LanguageParser, SymbolKind};
    use crate::parser::go::GoParser;
    use crate::parser::javascript::JavaScriptParser;
    use crate::parser::python::PythonParser;
    use crate::parser::rust::RustParser;

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
        assert_eq!(result.symbols[1].kind, SymbolKind::Method);
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
        assert!(result.symbols.iter().any(|s| s.name == "MyClass" && matches!(s.kind, SymbolKind::Class)));
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
        assert!(result.symbols.iter().any(|s| s.name == "MyStruct" && matches!(s.kind, SymbolKind::Struct)));
        assert!(result.imports.iter().any(|i| i.path.contains("std::collections")));
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
}
