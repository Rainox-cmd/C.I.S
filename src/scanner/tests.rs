#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_scan_empty_directory() {
        let dir = tempdir().unwrap();
        let scanner = Scanner::new(dir.path().to_path_buf(), true, true);
        let result = scanner.scan().unwrap();
        assert!(result.files.is_empty());
        assert_eq!(result.total_size, 0);
    }

    #[test]
    fn test_scan_finds_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("style.css"), "body {}").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("target").join("out.exe"), "").unwrap();

        let scanner = Scanner::new(dir.path().to_path_buf(), true, true);
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 2);
        assert!(result.files.iter().any(|f| f.ext == ".rs"));
        assert!(result.files.iter().any(|f| f.ext == ".css"));
        assert_eq!(result.language_counts.get("Rust"), Some(&1));
        assert_eq!(result.language_counts.get("CSS"), Some(&1));
    }

    #[test]
    fn test_scan_respects_ignore_dirs() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();
        fs::write(dir.path().join("node_modules").join("lib.js"), "").unwrap();
        fs::write(dir.path().join("app.py"), "print('hello')").unwrap();

        let scanner = Scanner::new(dir.path().to_path_buf(), true, true);
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].name, "app.py");
    }

    #[test]
    fn test_scan_skips_large_files() {
        let dir = tempdir().unwrap();
        let large_content = vec![0u8; 6 * 1024 * 1024]; // 6MB
        fs::write(dir.path().join("large.rs"), &large_content).unwrap();
        fs::write(dir.path().join("small.rs"), "fn main() {}").unwrap();

        let scanner = Scanner::new(dir.path().to_path_buf(), true, true);
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].name, "small.rs");
    }

    #[test]
    fn test_scan_categorizes_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();
        fs::write(dir.path().join("config.json"), "{}").unwrap();
        fs::write(dir.path().join("readme.md"), "# Title").unwrap();

        let scanner = Scanner::new(dir.path().to_path_buf(), true, true);
        let result = scanner.scan().unwrap();

        assert_eq!(result.category_counts.get("source"), Some(&1));
        assert_eq!(result.category_counts.get("config"), Some(&1));
        assert_eq!(result.category_counts.get("assets"), Some(&1));
    }

    #[test]
    fn test_scan_counts_lines() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "line1\nline2\nline3\n").unwrap();

        let scanner = Scanner::new(dir.path().to_path_buf(), true, true);
        let result = scanner.scan().unwrap();

        assert_eq!(result.files[0].lines, 3);
    }

    #[test]
    fn test_scan_computes_hash() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();

        let scanner = Scanner::new(dir.path().to_path_buf(), true, true);
        let result = scanner.scan().unwrap();

        assert!(!result.files[0].hash.is_empty());
        assert_eq!(result.files[0].hash.len(), 64); // SHA-256 hex length
    }

    #[test]
    fn test_incremental_scan_detects_changes() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();

        let scanner = Scanner::new(dir.path().to_path_buf(), true, true);
        let first = scanner.scan().unwrap();
        let hash = first.files[0].hash.clone();

        let mut previous = HashMap::new();
        previous.insert(first.files[0].rel_path.clone(), hash);

        let second = scanner.scan_incremental(&previous).unwrap();
        assert!(second.files.is_empty());

        fs::write(dir.path().join("main.py"), "print('world')").unwrap();
        let third = scanner.scan_incremental(&previous).unwrap();
        assert_eq!(third.files.len(), 1);
    }
}
