use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod generic;
pub mod go;
pub mod javascript;
pub mod python;
pub mod rust;

mod parser_engine;

pub use parser_engine::ParserEngine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    Variable,
    Constant,
    Interface,
    Struct,
    Enum,
    Trait,
    Module,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    pub path: String,
    pub is_relative: bool,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Export {
    pub name: String,
    pub kind: SymbolKind,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub symbols: Vec<Symbol>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub syntax_ok: bool,
    pub syntax_error: Option<String>,
    pub language: String,
}

pub trait LanguageParser {
    fn parse(&self, path: &Path, content: &str) -> ParseResult;
    fn language(&self) -> &str;
}

#[cfg(test)]
mod tests;
