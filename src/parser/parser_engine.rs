use super::{
    generic::GenericParser, go::GoParser, javascript::JavaScriptParser, python::PythonParser,
    rust::RustParser, LanguageParser, ParseResult,
};
use std::path::Path;

pub struct ParserEngine;

impl ParserEngine {
    pub fn parse_file(path: &Path, content: &str, language: &str) -> ParseResult {
        let parser: &dyn LanguageParser = match language {
            "Python" => &PythonParser,
            "JavaScript" | "TypeScript" => &JavaScriptParser,
            "Rust" => &RustParser,
            "Go" => &GoParser,
            _ => &GenericParser {
                language: language.to_string(),
            },
        };

        parser.parse(path, content)
    }
}
