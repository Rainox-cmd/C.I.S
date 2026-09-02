use super::*;
use crate::{config::Config, index::Index, project::Project};
use std::fs;
use tempfile::tempdir;

fn setup() -> (tempfile::TempDir, Project, Config, Index) {
    let dir = tempdir().unwrap();
    let project = Project::new(dir.path().to_path_buf()).unwrap();
    project.init().unwrap();
    let cfg = Config::default();
    let idx = Index::open(&project, &cfg).unwrap();
    (dir, project, cfg, idx)
}

#[test]
fn test_empty_analysis() {
    let (_dir, project, config, index) = setup();
    let report = AnalysisEngine::run(&project, &config, &index).unwrap();
    assert_eq!(report.issues.len(), 0);
    assert!(report.informational.is_empty());
}

#[test]
fn test_syntax_error_detection() {
    let (dir, project, config, index) = setup();
    let invalid_rust = "fn main() { let x = }"; // Syntax error
    
    let file_path = dir.path().join("main.rs");
    fs::write(&file_path, invalid_rust).unwrap();
    
    let report = AnalysisEngine::run(&project, &config, &index).unwrap();
    
    assert_eq!(report.issues.len(), 1);
    let issue = &report.issues[0];
    assert!(matches!(issue.category, IssueCategory::SyntaxError));
    assert_eq!(issue.file.as_deref(), Some("main.rs"));
}

#[test]
fn test_dependency_cycle() {
    let (_dir, project, config, index) = setup();
    
    // We mock a dependency cycle using existing index methods
    // Since has_cycle() is tested in graph_tests.rs, we can just insert edges
    use crate::scanner::FileRecord;
    let f1 = FileRecord {
        rel_path: "a.rs".into(),
        name: "a.rs".into(),
        ext: ".rs".into(),
        size: 10,
        language: "Rust".into(),
        category: "source".into(),
        lines: 1,
        hash: "h1".into(),
        mtime: 0.0,
    };
    let f2 = FileRecord {
        rel_path: "b.rs".into(),
        name: "b.rs".into(),
        ext: ".rs".into(),
        size: 10,
        language: "Rust".into(),
        category: "source".into(),
        lines: 1,
        hash: "h2".into(),
        mtime: 0.0,
    };
    index.upsert_files(&[f1, f2]).unwrap();
    
    // Create a cycle: a.rs -> b.rs -> a.rs
    index.upsert_dependencies("a.rs", &["b.rs".to_string()]).unwrap();
    let edges1 = vec![("b.rs".to_string(), "b.rs".to_string(), "import".to_string(), 1)];
    index.upsert_symbol_edges("a.rs", &edges1).unwrap();
    
    index.upsert_dependencies("b.rs", &["a.rs".to_string()]).unwrap();
    let edges2 = vec![("a.rs".to_string(), "a.rs".to_string(), "import".to_string(), 1)];
    index.upsert_symbol_edges("b.rs", &edges2).unwrap();
    
    let report = AnalysisEngine::run(&project, &config, &index).unwrap();
    
    // Expect at least the cycle warning
    assert!(report.issues.iter().any(|i| matches!(i.category, IssueCategory::DependencyCycle)));
}

#[test]
fn test_json_serialization() {
    let report = AnalysisReport {
        project_path: "/tmp/fake".to_string(),
        issues: vec![
            Issue {
                category: IssueCategory::ScanError,
                severity: Severity::Error,
                message: "A scan error".to_string(),
                file: None,
            }
        ],
        informational: vec!["Entry point".to_string()],
    };
    
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("ScanError"));
    assert!(json.contains("A scan error"));
    assert!(json.contains("informational"));
}
