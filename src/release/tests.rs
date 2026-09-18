use super::*;
use crate::project::Project;
use tempfile::tempdir;

fn setup() -> (tempfile::TempDir, Project) {
    let dir = tempdir().unwrap();
    let project = Project::new(dir.path().to_path_buf()).unwrap();
    project.init().unwrap();
    (dir, project)
}

#[test]
fn test_release_version() {
    let v = ReleaseManager::version();
    assert!(!v.is_empty());
    assert!(v.contains('.'));
}

#[test]
fn test_release_version_parts() {
    let (major, minor, patch) = ReleaseManager::version_parts();
    assert!(major > 0 || minor > 0 || patch > 0);
}

#[test]
fn test_release_build_name() {
    let name = ReleaseManager::build_release_name("windows", "x86_64", "msi");
    assert_eq!(name, "cis-windows-x86_64.msi");

    let name = ReleaseManager::build_release_name("linux", "x86_64", "deb");
    assert_eq!(name, "cis-linux-x86_64.deb");
}

#[test]
fn test_release_config_default() {
    let cfg = ReleaseConfig::default();
    assert!(!cfg.version.is_empty());
    assert_eq!(cfg.targets.len(), 5);
}

#[test]
fn test_release_config_targets() {
    let cfg = ReleaseConfig::default();

    let windows = cfg.targets.iter().find(|t| t.platform == "windows");
    assert!(windows.is_some());
    assert_eq!(windows.unwrap().package_format, "msi");

    let linux = cfg.targets.iter().find(|t| t.platform == "linux");
    assert!(linux.is_some());

    let macos = cfg.targets.iter().find(|t| t.platform == "macos");
    assert!(macos.is_some());
}

#[test]
fn test_release_validate_build() {
    let (_dir, project) = setup();
    let result = ReleaseManager::validate_build(&project);
    assert!(!result.unwrap());
}

#[test]
fn test_changelog_path() {
    let (_dir, project) = setup();
    let path = ReleaseManager::changelog_path(&project);
    assert!(path.is_absolute());
    assert_eq!(path, project.root.join("CHANGELOG.md"));
}

#[test]
fn test_generate_changelog() {
    let (_dir, project) = setup();
    
    // We are in a temp dir, not a git repo. git_log should gracefully return empty commits.
    let result = ReleaseManager::generate_changelog(&project);
    assert!(result.is_ok());
    let changelog = result.unwrap();
    assert!(changelog.contains("Changelog"));
    assert!(changelog.contains(&ReleaseManager::version()));
    // Should contain "Changes" heading
    assert!(changelog.contains("### Changes"));
}

#[test]
fn test_git_log_uses_project_root() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("nested").join("deep");
    std::fs::create_dir_all(&nested).unwrap();
    
    // Project root is nested/deep
    let project = Project::new(nested.clone()).unwrap();
    project.init().unwrap();
    
    // We ensure that we don't crash and we resolve correctly against the nested project root.
    // It should detect it's not a git repo and return empty string, rather than checking CWD.
    let result = ReleaseManager::generate_changelog(&project);
    assert!(result.is_ok());
}
