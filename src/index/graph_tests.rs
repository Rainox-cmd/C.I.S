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
fn test_graph_upsert_edges() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("main.py", "main.py", "Python"),
        make_file("utils.py", "utils.py", "Python"),
    ]).unwrap();

    let edges = vec![
        ("utils".to_string(), "utils.py".to_string(), "relative".to_string(), 3),
    ];
    idx.upsert_symbol_edges("main.py", &edges).unwrap();

    let forward = idx.get_forward_edges("main.py").unwrap();
    assert_eq!(forward.len(), 1);
    assert_eq!(forward[0].source_file, "main.py");
    assert_eq!(forward[0].target_file, "utils.py");
    assert_eq!(forward[0].source_symbol, Some("utils".to_string()));
    assert_eq!(forward[0].dep_type, "relative");
    assert_eq!(forward[0].line, 3);
}

#[test]
fn test_graph_reverse_edges() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("main.py", "main.py", "Python"),
        make_file("utils.py", "utils.py", "Python"),
    ]).unwrap();

    let edges = vec![
        ("utils".to_string(), "utils.py".to_string(), "relative".to_string(), 3),
    ];
    idx.upsert_symbol_edges("main.py", &edges).unwrap();

    let reverse = idx.get_reverse_edges("utils.py").unwrap();
    assert_eq!(reverse.len(), 1);
    assert_eq!(reverse[0].source_file, "main.py");
    assert_eq!(reverse[0].target_file, "utils.py");
}

#[test]
fn test_graph_forward_edges_empty() {
    let (_dir, _project, _cfg, idx) = setup();
    let forward = idx.get_forward_edges("nonexistent.py").unwrap();
    assert!(forward.is_empty());
}

#[test]
fn test_graph_reverse_edges_empty() {
    let (_dir, _project, _cfg, idx) = setup();
    let reverse = idx.get_reverse_edges("nonexistent.py").unwrap();
    assert!(reverse.is_empty());
}

#[test]
fn test_graph_transitive_dependencies() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("a.py", "a.py", "Python"),
        make_file("b.py", "b.py", "Python"),
        make_file("c.py", "c.py", "Python"),
    ]).unwrap();

    idx.upsert_symbol_edges("a.py", &[("b".to_string(), "b.py".to_string(), "import".to_string(), 1)]).unwrap();
    idx.upsert_symbol_edges("b.py", &[("c".to_string(), "c.py".to_string(), "import".to_string(), 1)]).unwrap();

    let deps = idx.get_transitive_dependencies("a.py").unwrap();
    assert!(deps.contains(&"c.py".to_string()));
    assert!(deps.contains(&"b.py".to_string()));
}

#[test]
fn test_graph_reverse_dependencies() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("a.py", "a.py", "Python"),
        make_file("b.py", "b.py", "Python"),
        make_file("c.py", "c.py", "Python"),
    ]).unwrap();

    idx.upsert_symbol_edges("a.py", &[("b".to_string(), "b.py".to_string(), "import".to_string(), 1)]).unwrap();
    idx.upsert_symbol_edges("b.py", &[("c".to_string(), "c.py".to_string(), "import".to_string(), 1)]).unwrap();

    let affected = idx.get_reverse_dependencies("c.py").unwrap();
    assert!(affected.contains(&"a.py".to_string()));
    assert!(affected.contains(&"b.py".to_string()));
}

#[test]
fn test_graph_no_cycle() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("a.py", "a.py", "Python"),
        make_file("b.py", "b.py", "Python"),
    ]).unwrap();

    idx.upsert_symbol_edges("a.py", &[("b".to_string(), "b.py".to_string(), "import".to_string(), 1)]).unwrap();

    assert!(!idx.has_cycle().unwrap());
}

#[test]
fn test_graph_has_cycle() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("a.py", "a.py", "Python"),
        make_file("b.py", "b.py", "Python"),
    ]).unwrap();

    idx.upsert_symbol_edges("a.py", &[("b".to_string(), "b.py".to_string(), "import".to_string(), 1)]).unwrap();
    idx.upsert_symbol_edges("b.py", &[("a".to_string(), "a.py".to_string(), "import".to_string(), 1)]).unwrap();

    assert!(idx.has_cycle().unwrap());
}

#[test]
fn test_graph_entry_points() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("main.py", "main.py", "Python"),
        make_file("utils.py", "utils.py", "Python"),
        make_file("config.py", "config.py", "Python"),
    ]).unwrap();

    idx.upsert_symbol_edges("main.py", &[("utils".to_string(), "utils.py".to_string(), "import".to_string(), 1)]).unwrap();

    let entries = idx.get_entry_points().unwrap();
    let main_entry = entries.iter().find(|e| e.rel_path == "main.py");
    assert!(main_entry.is_some());
    assert_eq!(main_entry.unwrap().incoming_count, 0);

    let utils_entry = entries.iter().find(|e| e.rel_path == "utils.py");
    assert!(utils_entry.is_some());
    assert_eq!(utils_entry.unwrap().incoming_count, 1);
}

#[test]
fn test_graph_edges_cleared_on_rescan() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("main.py", "main.py", "Python"),
        make_file("utils.py", "utils.py", "Python"),
    ]).unwrap();

    idx.upsert_symbol_edges("main.py", &[
        ("utils".to_string(), "utils.py".to_string(), "import".to_string(), 3),
    ]).unwrap();
    assert_eq!(idx.get_forward_edges("main.py").unwrap().len(), 1);

    idx.upsert_symbol_edges("main.py", &[]).unwrap();
    assert_eq!(idx.get_forward_edges("main.py").unwrap().len(), 0);
}

#[test]
fn test_graph_edges_deleted_with_files() {
    let (_dir, _project, _cfg, idx) = setup();
    idx.upsert_files(&[
        make_file("main.py", "main.py", "Python"),
        make_file("utils.py", "utils.py", "Python"),
    ]).unwrap();

    idx.upsert_symbol_edges("main.py", &[
        ("utils".to_string(), "utils.py".to_string(), "import".to_string(), 3),
    ]).unwrap();

    idx.delete_files(&[make_file("utils.py", "utils.py", "Python").rel_path]).unwrap();

    let edges = idx.get_forward_edges("main.py").unwrap();
    assert!(edges.is_empty());
    let reverse = idx.get_reverse_edges("utils.py").unwrap();
    assert!(reverse.is_empty());
}
