use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const IGNORE_DIRS: &[&str] = &[
    ".git", "node_modules", "venv", "__pycache__", ".mypy_cache",
    "dist", "build", ".tox", ".pytest_cache", ".idea", ".vscode",
    "env", ".env", "site-packages", ".next", "coverage", "htmlcov",
    ".cache", "vendor", "bower_components", "jspm_packages",
    ".sass-cache", "target", "out", "output", "generated",
    ".gradle", ".mvn", "Pods", "DerivedData",
];

pub const LANGUAGE_MAP: &[(&str, &str)] = &[
    (".py", "Python"),
    (".js", "JavaScript"),
    (".jsx", "JavaScript"),
    (".ts", "TypeScript"),
    (".tsx", "TypeScript"),
    (".java", "Java"),
    (".c", "C"),
    (".cpp", "C++"),
    (".cc", "C++"),
    (".cs", "C#"),
    (".go", "Go"),
    (".rb", "Ruby"),
    (".php", "PHP"),
    (".html", "HTML"),
    (".css", "CSS"),
    (".json", "JSON"),
    (".yaml", "YAML"),
    (".yml", "YAML"),
    (".md", "Markdown"),
    (".sh", "Shell"),
    (".bash", "Shell"),
    (".toml", "TOML"),
    (".xml", "XML"),
    (".rs", "Rust"),
];

const FILE_CATEGORIES_SOURCE: &[&str] = &[".py", ".js", ".jsx", ".ts", ".tsx", ".java", ".c", ".cpp", ".cc", ".cs", ".go", ".rb", ".php", ".rs"];
const FILE_CATEGORIES_CONFIG: &[&str] = &[".json", ".yaml", ".yml", ".toml", ".xml", ".ini", ".cfg", ".env"];
const FILE_CATEGORIES_ASSETS: &[&str] = &[".html", ".css", ".md", ".txt", ".rst"];

const MAX_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub rel_path: String,
    pub name: String,
    pub ext: String,
    pub size: u64,
    pub language: String,
    pub category: String,
    pub lines: u64,
    pub hash: String,
    pub mtime: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub files: Vec<FileRecord>,
    pub language_counts: HashMap<String, u64>,
    pub category_counts: HashMap<String, u64>,
    pub total_size: u64,
    pub scan_errors: Vec<String>,
}

impl ScanResult {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            language_counts: HashMap::new(),
            category_counts: HashMap::new(),
            total_size: 0,
            scan_errors: Vec::new(),
        }
    }
}

pub struct Scanner {
    root: PathBuf,
    respect_gitignore: bool,
    respect_cisignore: bool,
}

impl Scanner {
    pub fn new(root: PathBuf, respect_gitignore: bool, respect_cisignore: bool) -> Self {
        Self {
            root,
            respect_gitignore,
            respect_cisignore,
        }
    }

    pub fn scan(&self) -> Result<ScanResult> {
        let mut result = ScanResult::new();
        let mut lang_counts: HashMap<String, u64> = HashMap::new();
        let mut cat_counts: HashMap<String, u64> = HashMap::new();

        let walker = WalkDir::new(&self.root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| Self::should_enter_dir(e, &self.root, self.respect_gitignore, self.respect_cisignore));

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    result.scan_errors.push(format!("walk error: {}", e));
                    continue;
                }
            };

            if entry.file_type().is_dir() {
                continue;
            }

            let path = entry.path();
            let ext = path.extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{}", e.to_lowercase()))
                .unwrap_or_default();

            let language = LANGUAGE_MAP.iter()
                .find(|(e, _)| *e == ext)
                .map(|(_, l)| *l)
                .unwrap_or("Other");

            let category = if FILE_CATEGORIES_SOURCE.contains(&ext.as_str()) {
                "source"
            } else if FILE_CATEGORIES_CONFIG.contains(&ext.as_str()) {
                "config"
            } else if FILE_CATEGORIES_ASSETS.contains(&ext.as_str()) {
                "assets"
            } else {
                "other"
            };

            let metadata = match fs::metadata(path) {
                Ok(m) => m,
                Err(e) => {
                    result.scan_errors.push(format!("metadata failed: {} — {}", path.display(), e));
                    continue;
                }
            };

            let size = metadata.len();
            if size > MAX_FILE_SIZE_BYTES {
                continue;
            }

            let rel_path = match path.strip_prefix(&self.root) {
                Ok(p) => p.to_string_lossy().replace("\\", "/"),
                Err(_) => path.to_string_lossy().replace("\\", "/"),
            };

            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let hash = match Self::hash_file(path) {
                Ok(h) => h,
                Err(e) => {
                    result.scan_errors.push(format!("hash failed: {} — {}", path.display(), e));
                    continue;
                }
            };

            let lines = Self::count_lines(path).unwrap_or(0);

            let mtime = metadata.modified()
                .ok()
                .and_then(|t| t.elapsed().ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            let record = FileRecord {
                rel_path,
                name,
                ext,
                size,
                language: language.to_string(),
                category: category.to_string(),
                lines,
                hash,
                mtime,
            };

            result.files.push(record);
            result.total_size += size;
            *lang_counts.entry(language.to_string()).or_insert(0) += 1;
            *cat_counts.entry(category.to_string()).or_insert(0) += 1;
        }

        result.language_counts = lang_counts;
        result.category_counts = cat_counts;
        Ok(result)
    }

    pub fn scan_incremental(&self, previous_hashes: &HashMap<String, String>) -> Result<ScanResult> {
        let result = self.scan()?;
        let mut new_files = ScanResult::new();
        let mut lang_counts: HashMap<String, u64> = HashMap::new();
        let mut cat_counts: HashMap<String, u64> = HashMap::new();

        for file in result.files {
            let is_new = !previous_hashes.contains_key(&file.rel_path);
            let is_changed = previous_hashes.get(&file.rel_path) != Some(&file.hash);

            if is_new || is_changed {
                new_files.files.push(file.clone());
                new_files.total_size += file.size;
                *lang_counts.entry(file.language.clone()).or_insert(0) += 1;
                *cat_counts.entry(file.category.clone()).or_insert(0) += 1;
            }
        }

        new_files.language_counts = lang_counts;
        new_files.category_counts = cat_counts;
        new_files.scan_errors = result.scan_errors;
        Ok(new_files)
    }

    fn should_enter_dir(entry: &walkdir::DirEntry, root: &Path, respect_gitignore: bool, respect_cisignore: bool) -> bool {
        let path = entry.path();
        
        if path == root {
            return true;
        }

        let name = entry.file_name().to_string_lossy();
        
        if IGNORE_DIRS.contains(&name.as_ref()) {
            return false;
        }

        if name.starts_with('.') && name != "." {
            return false;
        }

        if respect_gitignore || respect_cisignore {
            let ignore_file = if respect_cisignore {
                root.join(".cisignore")
            } else {
                root.join(".gitignore")
            };
            
            if ignore_file.exists() {
                if let Ok(content) = fs::read_to_string(&ignore_file) {
                    for line in content.lines() {
                        let pattern = line.trim();
                        if pattern.is_empty() || pattern.starts_with('#') {
                            continue;
                        }
                        if Self::matches_ignore_pattern(path, root, pattern) {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }

    fn matches_ignore_pattern(path: &Path, root: &Path, pattern: &str) -> bool {
        let rel = path.strip_prefix(root).unwrap_or(path);
        let rel_str = rel.to_string_lossy().replace("\\", "/");
        
        if pattern.ends_with('/') {
            let dir_pattern = &pattern[..pattern.len()-1];
            return rel_str.starts_with(dir_pattern) || rel_str == dir_pattern;
        }
        
        if pattern.contains('*') {
            let regex_pattern = pattern.replace(".", "\\.").replace("*", ".*");
            if let Ok(re) = regex::Regex::new(&regex_pattern) {
                return re.is_match(&rel_str);
            }
        }
        
        rel_str == pattern || rel_str.starts_with(pattern)
    }

    fn hash_file(path: &Path) -> Result<String> {
        let content = fs::read(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = hasher.finalize();
        Ok(format!("{:x}", hash))
    }

    fn count_lines(path: &Path) -> Result<u64> {
        let content = fs::read(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let count = content.iter().filter(|&&b| b == b'\n').count() as u64;
        Ok(count)
    }
}

impl Default for ScanResult {
    fn default() -> Self {
        Self::new()
    }
}

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

        let mut previous = std::collections::HashMap::new();
        previous.insert(first.files[0].rel_path.clone(), hash);

        let second = scanner.scan_incremental(&previous).unwrap();
        assert!(second.files.is_empty());

        fs::write(dir.path().join("main.py"), "print('world')").unwrap();
        let third = scanner.scan_incremental(&previous).unwrap();
        assert_eq!(third.files.len(), 1);
    }
}
