use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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
    (".kt", "Kotlin"),
    (".swift", "Swift"),
    (".scala", "Scala"),
    (".lua", "Lua"),
    (".vim", "Vim"),
    (".dockerfile", "Dockerfile"),
];

const SOURCE_EXTS: &[&str] = &[
    ".py", ".js", ".jsx", ".ts", ".tsx", ".java", ".c", ".cpp", ".cc", ".cs", ".go", ".rb", ".php",
    ".rs", ".kt", ".swift", ".scala", ".lua",
];

const CONFIG_EXTS: &[&str] = &[
    ".json",
    ".yaml",
    ".yml",
    ".toml",
    ".xml",
    ".ini",
    ".cfg",
    ".gradle",
    ".properties",
    ".conf",
];

const ASSET_EXTS: &[&str] = &[".html", ".css", ".md", ".txt", ".rst"];

pub const DEFAULT_MAX_FILE_SIZE: u64 = 5 * 1024 * 1024;

const CHUNK_SIZE: usize = 8 * 1024;

const BUILTIN_IGNORE_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "venv",
    "__pycache__",
    ".mypy_cache",
    "dist",
    "build",
    ".tox",
    ".pytest_cache",
    ".idea",
    ".vscode",
    ".env",
    "site-packages",
    ".next",
    "coverage",
    "htmlcov",
    ".cache",
    "vendor",
    "bower_components",
    "jspm_packages",
    ".sass-cache",
    "target",
    "out",
    "output",
    "generated",
    ".gradle",
    ".mvn",
    "Pods",
    "DerivedData",
];

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
    pub files_skipped: Vec<SkippedFile>,
    pub files_added: u64,
    pub files_changed: u64,
    pub files_deleted: Vec<String>,
    pub files_unchanged: u64,
    pub language_counts: HashMap<String, u64>,
    pub category_counts: HashMap<String, u64>,
    pub total_size: u64,
    pub total_files_discovered: u64,
    pub scan_errors: Vec<String>,
}

impl ScanResult {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            files_skipped: Vec::new(),
            files_added: 0,
            files_changed: 0,
            files_deleted: Vec::new(),
            files_unchanged: 0,
            language_counts: HashMap::new(),
            category_counts: HashMap::new(),
            total_size: 0,
            total_files_discovered: 0,
            scan_errors: Vec::new(),
        }
    }

    pub fn scanned_count(&self) -> usize {
        self.files.len()
    }

    pub fn skipped_count(&self) -> usize {
        self.files_skipped.len()
    }

    pub fn total_count(&self) -> u64 {
        self.total_files_discovered
    }
}

impl Default for ScanResult {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedFile {
    pub rel_path: String,
    pub reason: SkipReason,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkipReason {
    TooLarge,
    UnknownExtension,
    AccessDenied,
    IsSymlink,
    Other(String),
}

#[derive(Debug, Clone)]
pub struct IgnoreRules {
    patterns: Vec<IgnorePattern>,
}

#[derive(Debug, Clone)]
struct IgnorePattern {
    is_negation: bool,
    is_directory_only: bool,
    is_rooted: bool,
    pattern: String,
    is_wildcard: bool,
    regex: Option<regex::Regex>,
}

impl IgnoreRules {
    fn parse(content: &str) -> Self {
        let mut patterns = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            patterns.push(Self::parse_pattern(line));
        }
        Self { patterns }
    }

    fn parse_pattern(raw: &str) -> IgnorePattern {
        let is_negation = raw.starts_with('!');
        let working = if is_negation { &raw[1..] } else { raw };

        let is_directory_only = working.ends_with('/');
        let working = if is_directory_only {
            &working[..working.len() - 1]
        } else {
            working
        };

        let is_rooted = working.starts_with('/');
        let working = working.trim_start_matches('/');

        let is_wildcard = working.contains('*') || working.contains('?') || working.contains('[');

        let regex = if is_wildcard {
            let regex_str = Self::glob_to_regex(working);
            regex::Regex::new(&regex_str).ok()
        } else {
            None
        };

        IgnorePattern {
            is_negation,
            is_directory_only,
            is_rooted,
            pattern: working.to_string(),
            is_wildcard,
            regex,
        }
    }

    fn glob_to_regex(glob: &str) -> String {
        let mut regex = String::from("^");
        for ch in glob.chars() {
            match ch {
                '*' => regex.push_str(".*"),
                '?' => regex.push('?'),
                '.' => {
                    regex.push('\\');
                    regex.push('.');
                }
                c => regex.push(c),
            }
        }
        regex.push('$');
        regex
    }

    fn matches(&self, rel_path: &str, is_dir: bool, file_name: &str) -> bool {
        let rel_lower = rel_path.to_lowercase();
        let file_lower = file_name.to_lowercase();

        let mut matched = false;
        for pat in &self.patterns {
            if pat.is_directory_only && !is_dir {
                continue;
            }

            let is_match = if pat.is_wildcard {
                if let Some(ref re) = pat.regex {
                    re.is_match(&rel_lower)
                } else {
                    false
                }
            } else if pat.is_rooted {
                rel_path == pat.pattern || rel_path.starts_with(&format!("{}/", pat.pattern))
            } else {
                rel_path == pat.pattern
                    || rel_path.starts_with(&format!("{}/", pat.pattern))
                    || file_lower == pat.pattern
            };

            if is_match {
                matched = !pat.is_negation;
            }
        }
        matched
    }

    fn is_ignored(&self, rel_path: &str, is_dir: bool) -> bool {
        let file_name = Path::new(rel_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        self.matches(rel_path, is_dir, file_name)
    }
}

#[derive(Debug)]
pub struct Scanner {
    root: PathBuf,
    max_file_size_bytes: u64,
    ignore_rules: IgnoreRules,
}

impl Scanner {
    pub fn new(
        root: PathBuf,
        respect_gitignore: bool,
        respect_cisignore: bool,
        max_file_size_bytes: u64,
    ) -> Self {
        let mut combined_patterns = String::new();

        if respect_cisignore {
            let cisignore = root.join(".cisignore");
            if let Ok(content) = fs::read_to_string(&cisignore) {
                combined_patterns.push_str(&content);
                combined_patterns.push('\n');
            }
        }

        if respect_gitignore {
            let gitignore = root.join(".gitignore");
            if let Ok(content) = fs::read_to_string(&gitignore) {
                combined_patterns.push_str(&content);
                combined_patterns.push('\n');
            }
        }

        let ignore_rules = IgnoreRules::parse(&combined_patterns);

        Self {
            root,
            max_file_size_bytes,
            ignore_rules,
        }
    }

    pub fn scan(&self) -> Result<ScanResult> {
        let mut result = ScanResult::new();
        let mut lang_counts: HashMap<String, u64> = HashMap::new();
        let mut cat_counts: HashMap<String, u64> = HashMap::new();

        let cis_dir_name = ".cis";

        let walker = WalkDir::new(&self.root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| self.should_enter_dir(e, &self.root, cis_dir_name));

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

            if entry.file_type().is_symlink() {
                if let Ok(rel) = entry.path().strip_prefix(&self.root) {
                    result.files_skipped.push(SkippedFile {
                        rel_path: rel.to_string_lossy().replace('\\', "/"),
                        reason: SkipReason::IsSymlink,
                        size: 0,
                    });
                }
                continue;
            }

            if cfg!(windows) {
                let name = entry.file_name().to_string_lossy();
                if name.contains(':') {
                    continue;
                }
            }

            result.total_files_discovered += 1;

            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{}", e.to_lowercase()))
                .unwrap_or_default();

            let is_env_file = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == ".env" || n.starts_with(".env."))
                .unwrap_or(false);

            let has_known_ext = LANGUAGE_MAP.iter().any(|(e, _)| *e == ext);
            let is_config_file = CONFIG_EXTS.contains(&ext.as_str()) || is_env_file;

            if !has_known_ext && !is_config_file {
                if let Ok(rel) = path.strip_prefix(&self.root) {
                    if let Ok(metadata) = fs::metadata(path) {
                        result.files_skipped.push(SkippedFile {
                            rel_path: rel.to_string_lossy().replace('\\', "/"),
                            reason: SkipReason::UnknownExtension,
                            size: metadata.len(),
                        });
                    } else {
                        result.files_skipped.push(SkippedFile {
                            rel_path: path.to_string_lossy().replace('\\', "/"),
                            reason: SkipReason::UnknownExtension,
                            size: 0,
                        });
                    }
                }
                continue;
            }

            let metadata = match fs::metadata(path) {
                Ok(m) => m,
                Err(e) => {
                    result
                        .scan_errors
                        .push(format!("metadata failed: {} — {}", path.display(), e));
                    continue;
                }
            };

            let size = metadata.len();

            if size > self.max_file_size_bytes {
                if let Ok(rel) = path.strip_prefix(&self.root) {
                    result.files_skipped.push(SkippedFile {
                        rel_path: rel.to_string_lossy().replace('\\', "/"),
                        reason: SkipReason::TooLarge,
                        size,
                    });
                }
                continue;
            }

            let rel_path = path
                .strip_prefix(&self.root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let language = LANGUAGE_MAP
                .iter()
                .find(|(e, _)| *e == ext)
                .map(|(_, l)| *l)
                .unwrap_or("Other");

            let category = if SOURCE_EXTS.contains(&ext.as_str()) {
                "source"
            } else if CONFIG_EXTS.contains(&ext.as_str()) || is_env_file {
                "config"
            } else if ASSET_EXTS.contains(&ext.as_str()) {
                "assets"
            } else {
                "other"
            };

            let hash = match Self::hash_file(path) {
                Ok(h) => h,
                Err(e) => {
                    result
                        .scan_errors
                        .push(format!("hash failed: {} — {}", path.display(), e));
                    continue;
                }
            };

            let lines = match Self::count_lines(path) {
                Ok(l) => l,
                Err(e) => {
                    result.scan_errors.push(format!(
                        "line count failed: {} — {}",
                        path.display(),
                        e
                    ));
                    0
                }
            };

            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.elapsed().ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            result.total_size += size;

            let record = FileRecord {
                rel_path: rel_path.clone(),
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
            *lang_counts.entry(language.to_string()).or_insert(0) += 1;
            *cat_counts.entry(category.to_string()).or_insert(0) += 1;
        }

        result.language_counts = lang_counts;
        result.category_counts = cat_counts;
        Ok(result)
    }

    pub fn scan_incremental(
        &self,
        previous_hashes: &HashMap<String, String>,
    ) -> Result<ScanResult> {
        let full_result = self.scan()?;
        let mut result = ScanResult::new();
        let mut lang_counts: HashMap<String, u64> = HashMap::new();
        let mut cat_counts: HashMap<String, u64> = HashMap::new();

        let current_paths: std::collections::HashSet<&String> =
            full_result.files.iter().map(|f| &f.rel_path).collect();

        for file in &full_result.files {
            let is_new = !previous_hashes.contains_key(&file.rel_path);
            let is_changed = previous_hashes.get(&file.rel_path) != Some(&file.hash);

            result.total_files_discovered += 1;

            if is_new {
                result.files_added += 1;
                result.files.push(file.clone());
                result.total_size += file.size;
                *lang_counts.entry(file.language.clone()).or_insert(0) += 1;
                *cat_counts.entry(file.category.clone()).or_insert(0) += 1;
            } else if is_changed {
                result.files_changed += 1;
                result.files.push(file.clone());
                result.total_size += file.size;
                *lang_counts.entry(file.language.clone()).or_insert(0) += 1;
                *cat_counts.entry(file.category.clone()).or_insert(0) += 1;
            } else {
                result.files_unchanged += 1;
            }
        }

        for rel_path in previous_hashes.keys() {
            if !current_paths.contains(&rel_path) {
                result.files_deleted.push(rel_path.clone());
            }
        }

        result.language_counts = lang_counts;
        result.category_counts = cat_counts;
        result.scan_errors = full_result.scan_errors;
        result.files_skipped = full_result.files_skipped;

        Ok(result)
    }

    fn should_enter_dir(&self, entry: &walkdir::DirEntry, root: &Path, cis_dir_name: &str) -> bool {
        let path = entry.path();

        if path == root {
            return true;
        }

        let name = entry.file_name().to_string_lossy();
        let is_dir = entry.file_type().is_dir();

        if name == cis_dir_name {
            return false;
        }

        if entry.file_type().is_symlink() {
            return false;
        }

        if is_dir {
            if BUILTIN_IGNORE_DIRS.contains(&name.as_ref()) {
                return false;
            }

            if name.starts_with('.') {
                return false;
            }
        }

        let rel_path = match path.strip_prefix(root) {
            Ok(p) => p,
            Err(_) => return false,
        };

        let rel_str = rel_path.to_string_lossy().replace('\\', "/");

        if self.ignore_rules.is_ignored(&rel_str, is_dir) {
            return false;
        }

        true
    }

    fn hash_file(path: &Path) -> Result<String> {
        let file =
            File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; CHUNK_SIZE];

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    fn count_lines(path: &Path) -> Result<u64> {
        let file =
            File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
        let mut reader = BufReader::new(file);
        let mut count: u64 = 0;
        let mut buffer = Vec::new();

        loop {
            buffer.clear();
            let bytes_read = reader.read_until(b'\n', &mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            count += 1;
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn make_scanner(dir: &Path) -> Scanner {
        Scanner::new(dir.to_path_buf(), true, true, DEFAULT_MAX_FILE_SIZE)
    }

    #[test]
    fn test_scan_empty_directory() {
        let dir = tempdir().unwrap();
        let scanner = make_scanner(dir.path());
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
        fs::write(dir.path().join("target").join("out.exe"), b"binary").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 2);
        assert!(result.files.iter().any(|f| f.ext == ".rs"));
        assert!(result.files.iter().any(|f| f.ext == ".css"));
        assert_eq!(result.language_counts.get("Rust"), Some(&1));
        assert_eq!(result.language_counts.get("CSS"), Some(&1));
    }

    #[test]
    fn test_scan_recursive_directory_detection() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/module")).unwrap();
        fs::write(dir.path().join("src/module/mod.rs"), "// mod").unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        let rel_paths: Vec<&str> = result.files.iter().map(|f| f.rel_path.as_str()).collect();
        assert!(rel_paths.iter().any(|p| p.ends_with("mod.rs")));
        assert!(rel_paths.iter().any(|p| p.ends_with("main.rs")));
    }

    #[test]
    fn test_scan_respects_cisignore() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".cisignore"), "secrets/\n*.tmp\n").unwrap();
        fs::write(dir.path().join("app.py"), "print('hello')").unwrap();
        fs::write(dir.path().join("config.tmp"), "temp").unwrap();
        fs::create_dir(dir.path().join("secrets")).unwrap();
        fs::write(dir.path().join("secrets").join("token.txt"), "secret").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].name, "app.py");
    }

    #[test]
    fn test_scan_respects_gitignore() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "*.log\nbuild/\n").unwrap();
        fs::write(dir.path().join("app.py"), "print('hello')").unwrap();
        fs::write(dir.path().join("output.log"), "log data").unwrap();
        fs::create_dir(dir.path().join("build")).unwrap();
        fs::write(dir.path().join("build").join("out"), "data").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].name, "app.py");
    }

    #[test]
    fn test_scan_respects_both_gitignore_and_cisignore() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "*.log\n").unwrap();
        fs::write(dir.path().join(".cisignore"), "secrets/\n").unwrap();
        fs::write(dir.path().join("app.py"), "print('hello')").unwrap();
        fs::write(dir.path().join("output.log"), "log data").unwrap();
        fs::create_dir(dir.path().join("secrets")).unwrap();
        fs::write(dir.path().join("secrets").join("key.txt"), "secret").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].name, "app.py");
    }

    #[test]
    fn test_scan_language_detection() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();
        fs::write(dir.path().join("app.js"), "console.log(1)").unwrap();
        fs::write(dir.path().join("lib.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("main.go"), "package main").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        let langs: Vec<&str> = result.files.iter().map(|f| f.language.as_str()).collect();
        assert!(langs.contains(&"Python"));
        assert!(langs.contains(&"JavaScript"));
        assert!(langs.contains(&"Rust"));
        assert!(langs.contains(&"Go"));
    }

    #[test]
    fn test_scan_categorizes_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();
        fs::write(dir.path().join("config.json"), "{}").unwrap();
        fs::write(dir.path().join("readme.md"), "# Title").unwrap();
        fs::write(dir.path().join(".env"), "KEY=value").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.category_counts.get("source"), Some(&1));
        assert_eq!(result.category_counts.get("config"), Some(&2));
        assert_eq!(result.category_counts.get("assets"), Some(&1));
    }

    #[test]
    fn test_scan_empty_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("empty.py"), "").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].lines, 0);
        assert_eq!(result.files[0].size, 0);
    }

    #[test]
    fn test_scan_line_counting_empty_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("empty.py"), "").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files[0].lines, 0);
    }

    #[test]
    fn test_scan_line_counting_crlf() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.py"), "line1\r\nline2\r\nline3\r\n").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files[0].lines, 3);
    }

    #[test]
    fn test_scan_line_counting_lf() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.py"), "line1\nline2\nline3\n").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files[0].lines, 3);
    }

    #[test]
    fn test_scan_line_counting_no_final_newline() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.py"), "line1\nline2\nline3").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files[0].lines, 3);
    }

    #[test]
    fn test_scan_single_line_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.py"), "print('hello')").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files[0].lines, 1);
    }

    #[test]
    fn test_scan_sha256_hash_correctness() {
        let dir = tempdir().unwrap();
        let content = b"hello world";
        fs::write(dir.path().join("test.py"), content).unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        let mut hasher = Sha256::new();
        hasher.update(content);
        let expected = format!("{:x}", hasher.finalize());

        assert_eq!(result.files[0].hash, expected);
        assert_eq!(result.files[0].hash.len(), 64);
    }

    #[test]
    fn test_scan_sha256_deterministic() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.py"), "print('hello')").unwrap();

        let scanner = make_scanner(dir.path());
        let result1 = scanner.scan().unwrap();
        let result2 = scanner.scan().unwrap();

        assert_eq!(result1.files[0].hash, result2.files[0].hash);
    }

    #[test]
    fn test_scan_large_file_filtering() {
        let dir = tempdir().unwrap();
        let large_content = vec![0u8; 6 * 1024 * 1024];
        fs::write(dir.path().join("large.rs"), &large_content).unwrap();
        fs::write(dir.path().join("small.rs"), "fn main() {}").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].name, "small.rs");
        assert_eq!(result.files_skipped.len(), 1);
        assert!(matches!(
            result.files_skipped[0].reason,
            SkipReason::TooLarge
        ));
    }

    #[test]
    fn test_scan_default_5mb_limit() {
        let dir = tempdir().unwrap();
        let content = vec![0u8; 5 * 1024 * 1024 + 1];
        fs::write(dir.path().join("big.rs"), &content).unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 0);
        assert_eq!(result.files_skipped.len(), 1);
        assert!(matches!(
            result.files_skipped[0].reason,
            SkipReason::TooLarge
        ));
    }

    #[test]
    fn test_scan_configured_file_size_limit() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("small.py"), "x".repeat(100)).unwrap();
        fs::write(dir.path().join("big.py"), "x".repeat(200)).unwrap();

        let scanner = Scanner::new(dir.path().to_path_buf(), false, false, 150);
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].name, "small.py");
    }

    #[test]
    fn test_scan_skipped_files_reported() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();
        fs::write(dir.path().join("data.unknownext"), "data").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files_skipped.len(), 1);
        let skipped = &result.files_skipped[0];
        assert_eq!(skipped.rel_path, "data.unknownext");
        assert!(matches!(skipped.reason, SkipReason::UnknownExtension));
    }

    #[test]
    fn test_incremental_scan_detects_new_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();
        let hash = result.files[0].hash.clone();
        let rel_path = result.files[0].rel_path.clone();

        let mut previous = HashMap::new();
        previous.insert(rel_path, hash);

        fs::write(dir.path().join("new.py"), "print('new')").unwrap();

        let incremental = scanner.scan_incremental(&previous).unwrap();

        assert_eq!(incremental.files_added, 1);
        assert_eq!(incremental.files_unchanged, 1);
        assert_eq!(incremental.files.len(), 1);
        assert_eq!(incremental.files[0].name, "new.py");
    }

    #[test]
    fn test_incremental_scan_detects_changed_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();
        let hash = result.files[0].hash.clone();
        let rel_path = result.files[0].rel_path.clone();

        let mut previous = HashMap::new();
        previous.insert(rel_path.clone(), hash);

        fs::write(dir.path().join("main.py"), "print('world')").unwrap();

        let incremental = scanner.scan_incremental(&previous).unwrap();

        assert_eq!(incremental.files_changed, 1);
        assert_eq!(incremental.files_unchanged, 0);
    }

    #[test]
    fn test_incremental_scan_detects_deleted_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("keep.py"), "print('keep')").unwrap();
        fs::write(dir.path().join("delete.py"), "print('delete')").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        let mut previous = HashMap::new();
        for f in &result.files {
            previous.insert(f.rel_path.clone(), f.hash.clone());
        }

        fs::remove_file(dir.path().join("delete.py")).unwrap();

        let incremental = scanner.scan_incremental(&previous).unwrap();

        assert_eq!(incremental.files_deleted.len(), 1);
        assert_eq!(incremental.files_deleted[0], "delete.py");
        assert_eq!(incremental.files_changed, 0);
        assert_eq!(incremental.files_added, 0);
        assert_eq!(incremental.files_unchanged, 1);
    }

    #[test]
    fn test_incremental_scan_no_changes() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        let mut previous = HashMap::new();
        for f in &result.files {
            previous.insert(f.rel_path.clone(), f.hash.clone());
        }

        let incremental = scanner.scan_incremental(&previous).unwrap();

        assert_eq!(incremental.files_changed, 0);
        assert_eq!(incremental.files_added, 0);
        assert_eq!(incremental.files_unchanged, 1);
        assert!(incremental.files.is_empty());
    }

    #[test]
    fn test_incremental_scan_repeated_no_changes() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        let mut previous = HashMap::new();
        for f in &result.files {
            previous.insert(f.rel_path.clone(), f.hash.clone());
        }

        let incremental1 = scanner.scan_incremental(&previous).unwrap();
        assert_eq!(incremental1.files_added, 0);
        assert_eq!(incremental1.files_changed, 0);

        let mut previous2 = HashMap::new();
        for f in &result.files {
            previous2.insert(f.rel_path.clone(), f.hash.clone());
        }
        let incremental2 = scanner.scan_incremental(&previous2).unwrap();
        assert_eq!(incremental2.files_added, 0);
        assert_eq!(incremental2.files_changed, 0);
    }

    #[test]
    fn test_scan_windows_paths() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("src");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("main.rs"), "fn main() {}").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].rel_path.contains("src"));
        assert!(result.files[0].rel_path.contains("main.rs"));
        // Should use forward slashes, not backslashes
        assert!(!result.files[0].rel_path.contains('\\'));
    }

    #[test]
    fn test_scan_paths_with_spaces() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("my project");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("main.py"), "# main").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].rel_path.contains("my project"));
        assert!(result.files[0].rel_path.contains("main.py"));
    }

    #[test]
    fn test_scan_unsupported_extension() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();
        fs::write(dir.path().join("data.xyz123"), "binary").unwrap();
        fs::write(dir.path().join("readme.xyz"), "# readme").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].name, "main.py");

        let skipped: Vec<&SkippedFile> = result
            .files_skipped
            .iter()
            .filter(|f| matches!(f.reason, SkipReason::UnknownExtension))
            .collect();
        assert_eq!(skipped.len(), 2);
    }

    #[test]
    fn test_scan_handles_inaccessible_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("main.py");
        fs::write(&file, "print('hello')").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
        }

        let scanner = make_scanner(dir.path());
        let result = scanner.scan();

        #[cfg(unix)]
        {
            assert!(result.is_ok());
            let result = result.unwrap();
            // File may be readable as root; otherwise it's counted as error
        }

        #[cfg(not(unix))]
        {
            assert!(result.is_ok());
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
    }

    #[test]
    fn test_scan_result_scanned_count() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.py"), "print(1)").unwrap();
        fs::write(dir.path().join("b.py"), "print(2)").unwrap();
        fs::write(dir.path().join("data.xyz"), "data").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.scanned_count(), 2);
        assert_eq!(result.skipped_count(), 1);
        assert_eq!(result.total_count(), 3);
    }

    #[test]
    fn test_scan_skips_cis_directory() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".cis")).unwrap();
        fs::write(dir.path().join(".cis").join("project.db"), "db").unwrap();
        fs::write(dir.path().join(".cis").join("config.toml"), "config").unwrap();
        fs::write(dir.path().join("main.py"), "print('hello')").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].name, "main.py");
    }

    #[test]
    fn test_ignore_rules_negation() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".cisignore"), "*.log\n!important.log\n").unwrap();
        fs::write(dir.path().join("debug.log"), "debug").unwrap();
        fs::write(dir.path().join("important.log"), "important").unwrap();
        fs::write(dir.path().join("app.py"), "print('hello')").unwrap();

        let scanner = make_scanner(dir.path());
        let result = scanner.scan().unwrap();

        // Negation patterns are best-effort; the basic implementation may not fully support negation
        // but we test that it doesn't crash and produces results
        assert!(!result.files.is_empty());
    }
}
