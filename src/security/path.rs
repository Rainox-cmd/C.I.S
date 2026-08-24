use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum PathContainmentError {
    #[error("path could not be canonicalized: {path}")]
    Canonicalization {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("path '{path}' is not contained within '{base}'")]
    NotContained { path: String, base: String },
    #[error("path contains a symlink that escapes the base directory")]
    SymlinkEscape,
    #[error("path is absolute and does not fall within the project root")]
    AbsoluteOutside,
    #[error("path is empty")]
    Empty,
}

pub struct PathSecurity;

impl PathSecurity {
    pub fn is_path_safe(path: &Path, base: &Path) -> bool {
        check_path_containment(path, base).is_ok()
    }

    pub fn contains_path(base: &Path, path: &Path) -> bool {
        check_path_containment(path, base).is_ok()
    }
}

pub fn canonicalize_path(path: &Path) -> Result<PathBuf, PathContainmentError> {
    let canonical = std::fs::canonicalize(path).map_err(|e| {
        PathContainmentError::Canonicalization {
            path: path.to_string_lossy().to_string(),
            source: e,
        }
    })?;
    Ok(canonical)
}

pub fn check_path_containment(
    path: &Path,
    base: &Path,
) -> Result<PathBuf, PathContainmentError> {
    if path.as_os_str().is_empty() || base.as_os_str().is_empty() {
        return Err(PathContainmentError::Empty);
    }

    let canonical_base = canonicalize_path(base)?;
    let canonical_path = if path.is_absolute() {
        canonicalize_path(path)?
    } else {
        canonicalize_path(&base.join(path))?
    };

    if canonical_path == canonical_base {
        return Ok(canonical_path);
    }

    if !is_contained(&canonical_path, &canonical_base) {
        return Err(PathContainmentError::NotContained {
            path: canonical_path.to_string_lossy().to_string(),
            base: canonical_base.to_string_lossy().to_string(),
        });
    }

    Ok(canonical_path)
}

fn is_contained(path: &Path, base: &Path) -> bool {
    let norm_base = normalize_path(base);
    let norm_path = normalize_path(path);

    let base_count = norm_base.components().count();
    let path_starts_with_base: bool = norm_path
        .components()
        .take(base_count)
        .eq(norm_base.components());

    path_starts_with_base && norm_path.starts_with(&norm_base)
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::RootDir => {
                normalized.push(comp);
            }
            std::path::Component::Prefix(_) => {
                normalized.push(comp);
            }
            std::path::Component::Normal(name) => {
                normalized.push(name);
            }
        }
    }
    normalized
}

pub fn build_restricted_env(
    allowed_vars: &[&str],
    current_env: &[(String, String)],
) -> Vec<(String, String)> {
    let allowed: HashSet<&str> = allowed_vars.iter().cloned().collect();
    current_env
        .iter()
        .filter(|(k, _)| allowed.contains(k.as_str()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_check_path_containment_same_dir() {
        let dir = tempdir().unwrap();
        let result = check_path_containment(dir.path(), dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_path_containment_subdir() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        let result = check_path_containment(&subdir, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_path_containment_traversal_escape() {
        let dir = tempdir().unwrap();
        let escape = dir.path().join("..").join("..").join("evil");
        let result = check_path_containment(&escape, dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_check_path_containment_absolute_outside() {
        let dir = tempdir().unwrap();
        let outside = Path::new("/").join("etc");
        let result = check_path_containment(&outside, dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_check_path_containment_relative_inside() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("child");
        fs::create_dir(&subdir).unwrap();
        let result = check_path_containment(Path::new("child"), dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().to_string_lossy().contains("child"));
    }

    #[test]
    fn test_check_path_containment_relative_escape() {
        let dir = tempdir().unwrap();
        let result = check_path_containment(Path::new("../../../etc"), dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_path_with_parent_dir() {
        let p = Path::new("/a/b/../c");
        let normalized = normalize_path(p);
        assert_eq!(normalized, PathBuf::from("/a/c"));
    }

    #[test]
    fn test_normalize_path_with_cur_dir() {
        let p = Path::new("/a/./b");
        let normalized = normalize_path(p);
        assert_eq!(normalized, PathBuf::from("/a/b"));
    }

    #[test]
    fn test_is_contained_true() {
        let base = Path::new("/a/b");
        let path = Path::new("/a/b/c/d");
        assert!(is_contained(path, base));
    }

    #[test]
    fn test_is_contained_false() {
        let base = Path::new("/a/b");
        let path = Path::new("/a/bc/d");
        assert!(!is_contained(path, base));
    }

    #[test]
    fn test_is_contained_false_no_prefix() {
        let base = Path::new("/a/b");
        let path = Path::new("/x/y/z");
        assert!(!is_contained(path, base));
    }

    #[test]
    fn test_build_restricted_env() {
        let current_env = vec![
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("SECRET_KEY".to_string(), "supersecret".to_string()),
            ("HOME".to_string(), "/home/user".to_string()),
        ];
        let allowed = ["PATH", "HOME"];
        let result = build_restricted_env(&allowed, &current_env);
        let env_map: std::collections::HashMap<_, _> = result.into_iter().collect();
        assert!(env_map.contains_key("PATH"));
        assert!(env_map.contains_key("HOME"));
        assert!(!env_map.contains_key("SECRET_KEY"));
    }
}
