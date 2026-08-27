use super::*;
use std::process::Command;
use tempfile::tempdir;

fn setup_repo() -> (tempfile::TempDir, GitClient) {
    let dir = tempdir().unwrap();
    let repo_path = dir.path().to_path_buf();

    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    let client = GitClient::new(&repo_path).unwrap();
    (dir, client)
}

#[test]
fn test_git_is_repo() {
    let (_dir, client) = setup_repo();
    assert!(client.is_repo());
}

#[test]
fn test_git_not_repo() {
    let dir = tempdir().unwrap();
    let client = GitClient::new(dir.path()).unwrap();
    assert!(!client.is_repo());
}

#[test]
fn test_git_current_branch() {
    let (_dir, client) = setup_repo();
    let branch = client.current_branch().unwrap();
    assert_eq!(branch, "main");
}

#[test]
fn test_git_status_clean() {
    let (_dir, client) = setup_repo();

    std::fs::write(client.repo_path.join("test.py"), "print('hello')\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();

    let entries = client.status().unwrap();
    assert!(entries.is_empty());
    assert!(!client.is_dirty().unwrap());
}

#[test]
fn test_git_status_dirty() {
    let (_dir, client) = setup_repo();

    std::fs::write(client.repo_path.join("a.py"), "print('hello')\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();

    std::fs::write(client.repo_path.join("a.py"), "print('modified')\n").unwrap();
    std::fs::write(client.repo_path.join("b.py"), "print('new')\n").unwrap();

    let entries = client.status().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(client.is_dirty().unwrap());
}

#[test]
fn test_git_diff() {
    let (_dir, client) = setup_repo();

    std::fs::write(client.repo_path.join("a.py"), "print('hello')\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();

    std::fs::write(client.repo_path.join("a.py"), "print('modified')\n").unwrap();

    let diff = client.diff().unwrap();
    assert!(diff.contains("print('modified')"));
}

#[test]
fn test_git_commit_hash() {
    let (_dir, client) = setup_repo();

    std::fs::write(client.repo_path.join("a.py"), "print('hello')\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Test commit"])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();

    let hash = client.head_commit_hash().unwrap();
    assert!(!hash.is_empty());
    assert_eq!(hash.len(), 40);

    let short = client.short_commit_hash().unwrap();
    assert!(!short.is_empty());
}

#[test]
fn test_git_log() {
    let (_dir, client) = setup_repo();

    std::fs::write(client.repo_path.join("a.py"), "print('hello')\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "First commit"])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();

    std::fs::write(client.repo_path.join("b.py"), "print('world')\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Second commit"])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();

    let commits = client.log(10).unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].message, "Second commit");
    assert_eq!(commits[1].message, "First commit");
    assert!(!commits[0].author.is_empty());
}

#[test]
fn test_git_file_history() {
    let (_dir, client) = setup_repo();

    std::fs::write(client.repo_path.join("a.py"), "v1\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Commit 1 for a.py"])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();

    std::fs::write(client.repo_path.join("a.py"), "v2\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Commit 2 for a.py"])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();

    std::fs::write(client.repo_path.join("b.py"), "v1\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Commit 3 for b.py"])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();

    let history = client.file_history("a.py", 10).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].message, "Commit 2 for a.py");
    assert_eq!(history[1].message, "Commit 1 for a.py");

    let b_history = client.file_history("b.py", 10).unwrap();
    assert_eq!(b_history.len(), 1);
    assert_eq!(b_history[0].message, "Commit 3 for b.py");
}

#[test]
fn test_git_file_last_commit() {
    let (_dir, client) = setup_repo();

    std::fs::write(client.repo_path.join("a.py"), "v1\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Last commit for a.py"])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();

    let commit = client.file_last_commit("a.py").unwrap();
    assert!(commit.is_some());
    assert_eq!(commit.unwrap().message, "Last commit for a.py");
}

#[test]
fn test_git_file_last_commit_no_history() {
    let (_dir, client) = setup_repo();
    let commit = client.file_last_commit("nonexistent.py").unwrap();
    assert!(commit.is_none());
}

#[test]
fn test_git_link_file_commit() {
    let (_dir, client) = setup_repo();

    std::fs::write(client.repo_path.join("a.py"), "v1\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Test commit"])
        .current_dir(&client.repo_path)
        .output()
        .unwrap();

    let hash = client.link_file_commit("a.py").unwrap();
    assert!(hash.is_some());
    assert_eq!(hash.unwrap().len(), 7);
}
