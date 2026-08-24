use crate::parser::{LanguageParser, ParseResult};
use std::path::Path;

pub struct GenericParser {
    pub language: String,
}

impl LanguageParser for GenericParser {
    fn language(&self) -> &str {
        &self.language
    }

    fn parse(&self, _path: &Path, _content: &str) -> ParseResult {
        ParseResult {
            symbols: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            syntax_ok: true,
            syntax_error: None,
            language: self.language.clone(),
        }
    }
}
