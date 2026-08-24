#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_find_project_root_with_git() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        let root = find_project_root(dir.path());
        assert!(root.is_some());
        assert_eq!(root.unwrap(), dir.path());
    }

    #[test]
    fn test_find_project_root_with_cargo_toml() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();
        let root = find_project_root(dir.path());
        assert!(root.is_some());
        assert_eq!(root.unwrap(), dir.path());
    }

    #[test]
    fn test_find_project_root_with_package_json() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{\"name\": \"test\"}").unwrap();
        let root = find_project_root(dir.path());
        assert!(root.is_some());
        assert_eq!(root.unwrap(), dir.path());
    }

    #[test]
    fn test_find_project_root_walks_up() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("src").join("sub");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();
        let root = find_project_root(&subdir);
        assert!(root.is_some());
        assert_eq!(root.unwrap(), dir.path());
    }

    #[test]
    fn test_find_project_root_not_found() {
        let dir = tempdir().unwrap();
        let root = find_project_root(dir.path());
        assert!(root.is_none());
    }
}
