use super::*;
use crate::analysis::{Issue, IssueCategory, Severity};
use crate::config::Config;
use crate::project::Project;
use tempfile::tempdir;

fn setup() -> (tempfile::TempDir, Project, Config, Index) {
    let dir = tempdir().unwrap();
    let project = Project::new(dir.path().to_path_buf());
    let cfg = Config::default();
    let idx = Index::open(&project, &cfg).unwrap();
    (dir, project, cfg, idx)
}

#[test]
fn test_issue_persistence_insert() {
    let (_dir, _project, _cfg, idx) = setup();
    
    let issue = Issue {
        category: IssueCategory::ScanError,
        severity: Severity::Error,
        message: "Test scan error".to_string(),
        file: Some("src/main.rs".to_string()),
    };
    
    let changed = idx.save_issues(&[issue.clone()]).unwrap();
    assert_eq!(changed, 1);
    
    let issues = idx.get_issues(None).unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].status, "open");
    assert_eq!(issues[0].message, "Test scan error");
    assert_eq!(issues[0].category, IssueCategory::ScanError);
    assert_eq!(issues[0].identity, issue.identity());
}

#[test]
fn test_issue_persistence_deduplication() {
    let (_dir, _project, _cfg, idx) = setup();
    
    let issue = Issue {
        category: IssueCategory::ScanError,
        severity: Severity::Error,
        message: "Test scan error".to_string(),
        file: Some("src/main.rs".to_string()),
    };
    
    let changed1 = idx.save_issues(&[issue.clone()]).unwrap();
    assert_eq!(changed1, 1);
    
    // Save the exact same issue again
    let changed2 = idx.save_issues(&[issue.clone()]).unwrap();
    // It should update updated_at, but not insert a new row
    assert_eq!(changed2, 1); // 1 row updated
    
    let issues = idx.get_issues(None).unwrap();
    assert_eq!(issues.len(), 1);
}

#[test]
fn test_issue_persistence_resolution() {
    let (_dir, _project, _cfg, idx) = setup();
    
    let issue = Issue {
        category: IssueCategory::SyntaxError,
        severity: Severity::Error,
        message: "Syntax error".to_string(),
        file: Some("src/bad.rs".to_string()),
    };
    
    // First run sees the issue
    idx.save_issues(&[issue.clone()]).unwrap();
    
    let issues = idx.get_issues(Some("open")).unwrap();
    assert_eq!(issues.len(), 1);
    
    // Second run sees NO issues
    let changed = idx.save_issues(&[]).unwrap();
    assert_eq!(changed, 0); // No new inserts/updates
    
    let open_issues = idx.get_issues(Some("open")).unwrap();
    assert_eq!(open_issues.len(), 0);
    
    let resolved_issues = idx.get_issues(Some("resolved")).unwrap();
    assert_eq!(resolved_issues.len(), 1);
    assert_eq!(resolved_issues[0].identity, issue.identity());
}
