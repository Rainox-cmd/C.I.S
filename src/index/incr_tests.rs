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
fn test_incremental_single_file_change() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("a.py", "a.py", "Python"),
        make_file("b.py", "b.py", "Python"),
    ]).unwrap();

    idx.upsert_symbols("a.py", &[("func_a".to_string(), "Function".to_string(), 1, 0)]).unwrap();
    idx.upsert_symbols("b.py", &[("func_b".to_string(), "Function".to_string(), 1, 0)]).unwrap();

    let a_syms = idx.get_symbols_by_file("a.py").unwrap();
    assert_eq!(a_syms.len(), 1);
    assert_eq!(a_syms[0].name, "func_a");

    idx.upsert_symbols("a.py", &[("func_a_new".to_string(), "Function".to_string(), 5, 0)]).unwrap();

    let a_syms = idx.get_symbols_by_file("a.py").unwrap();
    assert_eq!(a_syms.len(), 1);
    assert_eq!(a_syms[0].name, "func_a_new");
    assert_eq!(a_syms[0].line, 5);

    let b_syms = idx.get_symbols_by_file("b.py").unwrap();
    assert_eq!(b_syms.len(), 1);
    assert_eq!(b_syms[0].name, "func_b");
}

#[test]
fn test_incremental_new_file() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("a.py", "a.py", "Python"),
    ]).unwrap();

    idx.upsert_symbols("a.py", &[("func_a".to_string(), "Function".to_string(), 1, 0)]).unwrap();

    assert_eq!(idx.get_symbols_by_file("a.py").unwrap().len(), 1);

    idx.upsert_files(&[
        make_file("a.py", "a.py", "Python"),
        make_file("b.py", "b.py", "Python"),
    ]).unwrap();

    idx.upsert_symbols("b.py", &[("func_b".to_string(), "Function".to_string(), 1, 0)]).unwrap();

    assert_eq!(idx.get_symbols_by_file("a.py").unwrap().len(), 1);
    assert_eq!(idx.get_symbols_by_file("b.py").unwrap().len(), 1);
}

#[test]
fn test_incremental_deleted_file() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("a.py", "a.py", "Python"),
        make_file("b.py", "b.py", "Python"),
    ]).unwrap();

    idx.upsert_symbols("a.py", &[("func_a".to_string(), "Function".to_string(), 1, 0)]).unwrap();
    idx.upsert_symbols("b.py", &[("func_b".to_string(), "Function".to_string(), 1, 0)]).unwrap();

    idx.upsert_symbol_edges("a.py", &[("b".to_string(), "b.py".to_string(), "import".to_string(), 1)]).unwrap();

    idx.delete_files(&["b.py".to_string()]).unwrap();

    assert_eq!(idx.get_symbols_by_file("a.py").unwrap().len(), 1);
    assert_eq!(idx.get_symbols_by_file("b.py").unwrap().len(), 0);
    assert!(idx.get_forward_edges("a.py").unwrap().is_empty());
}

#[test]
fn test_incremental_repeated_no_changes() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("a.py", "a.py", "Python"),
        make_file("b.py", "b.py", "Python"),
    ]).unwrap();

    idx.upsert_symbols("a.py", &[("func_a".to_string(), "Function".to_string(), 1, 0)]).unwrap();
    idx.upsert_symbols("b.py", &[("func_b".to_string(), "Function".to_string(), 1, 0)]).unwrap();

    idx.upsert_files(&[
        make_file("a.py", "a.py", "Python"),
        make_file("b.py", "b.py", "Python"),
    ]).unwrap();

    idx.upsert_symbols("a.py", &[("func_a".to_string(), "Function".to_string(), 1, 0)]).unwrap();
    idx.upsert_symbols("b.py", &[("func_b".to_string(), "Function".to_string(), 1, 0)]).unwrap();

    assert_eq!(idx.get_symbols_by_file("a.py").unwrap().len(), 1);
    assert_eq!(idx.get_symbols_by_file("b.py").unwrap().len(), 1);
}

#[test]
fn test_incremental_edge_update_on_reparse() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("a.py", "a.py", "Python"),
        make_file("b.py", "b.py", "Python"),
        make_file("c.py", "c.py", "Python"),
    ]).unwrap();

    idx.upsert_symbol_edges("a.py", &[
        ("b".to_string(), "b.py".to_string(), "import".to_string(), 1),
    ]).unwrap();

    assert_eq!(idx.get_forward_edges("a.py").unwrap().len(), 1);

    idx.upsert_symbol_edges("a.py", &[
        ("c".to_string(), "c.py".to_string(), "import".to_string(), 2),
    ]).unwrap();

    let edges = idx.get_forward_edges("a.py").unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].target_file, "c.py");
}

#[test]
fn test_incremental_hash_consistency() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("a.py", "a.py", "Python"),
    ]).unwrap();

    let hashes1 = idx.get_file_hashes().unwrap();
    assert!(hashes1.contains_key("a.py"));

    idx.upsert_files(&[
        make_file("a.py", "a.py", "Python"),
    ]).unwrap();

    let hashes2 = idx.get_file_hashes().unwrap();
    assert!(hashes2.contains_key("a.py"));
    assert_eq!(hashes1.len(), hashes2.len());
}

#[test]
fn test_incremental_config_defaults() {
    let cfg = Config::default();
    assert!(cfg.general.max_file_size_bytes > 0);
    assert!(cfg.general.max_files_per_batch > 0);
    assert!(cfg.general.parse_timeout_seconds > 0);
}

#[test]
fn test_incremental_config_set_max_files_per_batch() {
    let mut cfg = Config::default();
    cfg.set("general.max_files_per_batch", "100").unwrap();
    assert_eq!(cfg.general.max_files_per_batch, 100);
}

#[test]
fn test_incremental_config_set_parse_timeout() {
    let mut cfg = Config::default();
    cfg.set("general.parse_timeout_seconds", "30").unwrap();
    assert_eq!(cfg.general.parse_timeout_seconds, 30);
}
