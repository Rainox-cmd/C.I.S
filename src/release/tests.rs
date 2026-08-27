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
    assert!(path.to_string_lossy().ends_with("CHANGELOG.md"));
}

#[test]
fn test_generate_changelog() {
    let (_dir, project) = setup();
    let result = ReleaseManager::generate_changelog(&project);
    assert!(result.is_ok());
    let changelog = result.unwrap();
    assert!(changelog.contains("Changelog"));
    assert!(changelog.contains(&ReleaseManager::version()));
}
