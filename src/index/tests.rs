use super::*;
use crate::{config::Config, project::Project};
use tempfile::tempdir;

fn setup() -> (tempfile::TempDir, Project, Config, Index) {
    let dir = tempdir().unwrap();
    let project = Project::new(dir.path().to_path_buf()).unwrap();
    project.init().unwrap();
    let cfg = Config::default();
    let idx = Index::open(&project, &cfg).unwrap();
    (dir, project, cfg, idx)
}

fn make_file(rel_path: &str, name: &str, language: &str) -> FileRecord {
    FileRecord {
        rel_path: rel_path.to_string(),
        name: name.to_string(),
        ext: std::path::Path::new(name)
            .extension()
            .map(|e| format!(".{}", e.to_str().unwrap()))
            .unwrap_or_default(),
        size: 100,
        language: language.to_string(),
        category: "source".to_string(),
        lines: 10,
        hash: "hash1".to_string(),
        mtime: 1000.0,
    }
}

#[test]
fn test_index_open_creates_schema() {
    let (_dir, _project, _cfg, idx) = setup();
    let count: i64 = idx
        .conn
        .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_index_doctor() {
    let (_dir, _project, _cfg, idx) = setup();
    let result = idx.doctor();
    assert!(result.is_ok());
}

#[test]
fn test_index_status() {
    let (_dir, _project, _cfg, idx) = setup();
    let result = idx.status();
    assert!(result.is_ok());
}

#[test]
fn test_index_upsert_and_get_hashes() {
    let (_dir, _project, _cfg, idx) = setup();

    let files = vec![FileRecord {
        rel_path: "src/main.rs".to_string(),
        name: "main.rs".to_string(),
        ext: ".rs".to_string(),
        size: 100,
        language: "Rust".to_string(),
        category: "source".to_string(),
        lines: 10,
        hash: "abc123".to_string(),
        mtime: 1000.0,
    }];

    idx.upsert_files(&files).unwrap();

    let hashes = idx.get_file_hashes().unwrap();
    assert_eq!(hashes.get("src/main.rs"), Some(&"abc123".to_string()));
}

#[test]
fn test_index_integrity_check() {
    let (_dir, _project, _cfg, idx) = setup();
    assert!(idx.integrity_check().unwrap());
}

#[test]
fn test_index_fts5_table_exists() {
    let (_dir, _project, _cfg, idx) = setup();
    let result: rusqlite::Result<String> = idx.conn.query_row(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='files_fts'",
        [],
        |row| row.get(0),
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "files_fts");
}

#[test]
fn test_index_upsert_and_retrieve_symbols() {
    let (_dir, _project, _cfg, idx) = setup();

    let files = vec![make_file("src/main.rs", "main.rs", "Rust")];
    idx.upsert_files(&files).unwrap();

    let symbols: Vec<(String, String, u32, u32)> = vec![
        ("greet".to_string(), "Function".to_string(), 1, 0),
        ("MyStruct".to_string(), "Struct".to_string(), 5, 0),
    ];
    idx.upsert_symbols("src/main.rs", &symbols).unwrap();

    let retrieved = idx.get_symbols_by_file("src/main.rs").unwrap();
    assert_eq!(retrieved.len(), 2);
    assert_eq!(retrieved[0].name, "greet");
    assert_eq!(retrieved[0].symbol_type, "Function");
    assert_eq!(retrieved[0].line, 1);
    assert_eq!(retrieved[1].name, "MyStruct");
    assert_eq!(retrieved[1].symbol_type, "Struct");
    assert_eq!(retrieved[1].line, 5);
}

#[test]
fn test_index_find_symbols_by_name() {
    let (_dir, _project, _cfg, idx) = setup();

    let files = vec![make_file("src/main.rs", "main.rs", "Rust")];
    idx.upsert_files(&files).unwrap();

    let symbols: Vec<(String, String, u32, u32)> = vec![
        ("greet".to_string(), "Function".to_string(), 1, 0),
        ("main_func".to_string(), "Function".to_string(), 10, 0),
    ];
    idx.upsert_symbols("src/main.rs", &symbols).unwrap();

    let found = idx.find_symbols_by_name("greet").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "greet");

    let found_partial = idx.find_symbols_by_name("main").unwrap();
    assert!(found_partial.iter().any(|s| s.name == "main_func"));
}

#[test]
fn test_index_search_fts5() {
    let (_dir, _project, _cfg, idx) = setup();

    let files = vec![make_file("src/main.rs", "main.rs", "Rust")];
    idx.upsert_files(&files).unwrap();

    let symbols: Vec<(String, String, u32, u32)> = vec![
        ("greet".to_string(), "Function".to_string(), 1, 0),
        ("UserAuth".to_string(), "Struct".to_string(), 5, 0),
    ];
    idx.upsert_symbols("src/main.rs", &symbols).unwrap();

    let results = idx.search("greet").unwrap();
    assert!(results.iter().any(|r| r.name == "greet"));

    let results2 = idx.search("UserAuth").unwrap();
    assert!(results2.iter().any(|r| r.name == "UserAuth"));
}

#[test]
fn test_index_upsert_dependencies() {
    let (_dir, _project, _cfg, idx) = setup();

    let files = vec![
        make_file("src/main.rs", "main.rs", "Rust"),
        make_file("src/utils.rs", "utils.rs", "Rust"),
    ];
    idx.upsert_files(&files).unwrap();

    idx.upsert_dependencies("src/main.rs", &["src/utils.rs".to_string()])
        .unwrap();

    let deps = idx.get_dependencies("src/main.rs").unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].source_file, "src/main.rs");
    assert_eq!(deps[0].target_file, "src/utils.rs");
}

#[test]
fn test_index_incremental_update_symbols() {
    let (_dir, _project, _cfg, idx) = setup();

    let files = vec![make_file("src/main.rs", "main.rs", "Rust")];
    idx.upsert_files(&files).unwrap();

    let symbols1: Vec<(String, String, u32, u32)> = vec![
        ("old_func".to_string(), "Function".to_string(), 1, 0),
    ];
    idx.upsert_symbols("src/main.rs", &symbols1).unwrap();

    let symbols2: Vec<(String, String, u32, u32)> = vec![
        ("new_func".to_string(), "Function".to_string(), 2, 0),
    ];
    idx.upsert_symbols("src/main.rs", &symbols2).unwrap();

    let retrieved = idx.get_symbols_by_file("src/main.rs").unwrap();
    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].name, "new_func");
    assert_eq!(retrieved[0].line, 2);
}

#[test]
fn test_index_delete_files_cascades_symbols() {
    let (_dir, _project, _cfg, idx) = setup();

    let files = vec![make_file("src/main.rs", "main.rs", "Rust")];
    idx.upsert_files(&files).unwrap();

    let symbols: Vec<(String, String, u32, u32)> = vec![
        ("greet".to_string(), "Function".to_string(), 1, 0),
    ];
    idx.upsert_symbols("src/main.rs", &symbols).unwrap();

    idx.delete_files(&["src/main.rs".to_string()]).unwrap();

    let retrieved = idx.get_symbols_by_file("src/main.rs").unwrap();
    assert_eq!(retrieved.len(), 0);
}

#[test]
fn test_index_delete_files_cascades_dependencies() {
    let (_dir, _project, _cfg, idx) = setup();

    let files = vec![
        make_file("src/main.rs", "main.rs", "Rust"),
        make_file("src/utils.rs", "utils.rs", "Rust"),
    ];
    idx.upsert_files(&files).unwrap();

    idx.upsert_dependencies("src/main.rs", &["src/utils.rs".to_string()])
        .unwrap();

    idx.delete_files(&["src/utils.rs".to_string()]).unwrap();

    let deps = idx.get_dependencies("src/main.rs").unwrap();
    assert_eq!(deps.len(), 0);
}

#[test]
fn test_index_upsert_symbols_for_multiple_files() {
    let (_dir, _project, _cfg, idx) = setup();

    let files = vec![
        make_file("src/main.rs", "main.rs", "Rust"),
        make_file("src/lib.rs", "lib.rs", "Rust"),
    ];
    idx.upsert_files(&files).unwrap();

    let batch_main: SymbolBatch = vec![("greet".to_string(), "Function".to_string(), 1, 0)];
    let batch_lib: SymbolBatch = vec![("Helper".to_string(), "Struct".to_string(), 3, 0)];
    let batch: Vec<(&str, &SymbolBatch)> = vec![
        ("src/main.rs", &batch_main),
        ("src/lib.rs", &batch_lib),
    ];

    idx.upsert_symbols_for_files(&batch).unwrap();

    let main_symbols = idx.get_symbols_by_file("src/main.rs").unwrap();
    assert_eq!(main_symbols.len(), 1);
    assert_eq!(main_symbols[0].name, "greet");

    let lib_symbols = idx.get_symbols_by_file("src/lib.rs").unwrap();
    assert_eq!(lib_symbols.len(), 1);
    assert_eq!(lib_symbols[0].name, "Helper");
}