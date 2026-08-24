use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

use crate::project::Project;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub scanner: ScannerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub project_name: String,
    pub ignore_dirs: Vec<String>,
    pub max_file_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerConfig {
    pub languages: Vec<String>,
    pub enable_incremental: bool,
    pub respect_gitignore: bool,
    pub respect_cisignore: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub wal_mode: bool,
    pub journal_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub default_timeout_seconds: u64,
    pub max_output_bytes: u64,
    pub command_allowlist: Vec<String>,
    pub command_denylist: Vec<String>,
    pub require_confirmation_for_risky: bool,
    pub restrict_to_project_root: bool,
    pub redact_secrets: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                project_name: String::new(),
                ignore_dirs: vec![
                    "node_modules".into(),
                    "target".into(),
                    ".git".into(),
                    "dist".into(),
                    "build".into(),
                    ".venv".into(),
                    "venv".into(),
                    "__pycache__".into(),
                ],
                max_file_size_bytes: 5 * 1024 * 1024,
            },
            scanner: ScannerConfig {
                languages: vec![
                    "python".into(),
                    "javascript".into(),
                    "typescript".into(),
                    "rust".into(),
                    "go".into(),
                    "java".into(),
                    "c".into(),
                    "cpp".into(),
                    "ruby".into(),
                    "php".into(),
                    "html".into(),
                    "css".into(),
                ],
                enable_incremental: true,
                respect_gitignore: true,
                respect_cisignore: true,
            },
            database: DatabaseConfig {
                wal_mode: true,
                journal_mode: "WAL".into(),
            },
            security: SecurityConfig {
                default_timeout_seconds: 30,
                max_output_bytes: 1024 * 1024,
                command_allowlist: vec![
                    "python".into(),
                    "node".into(),
                    "cargo".into(),
                    "go".into(),
                    "java".into(),
                    "rustc".into(),
                    "npm".into(),
                ],
                command_denylist: vec![
                    "rm -rf".into(),
                    "del /s".into(),
                    "format".into(),
                    "shutdown".into(),
                    "diskpart".into(),
                    "fdisk".into(),
                ],
                require_confirmation_for_risky: true,
                restrict_to_project_root: true,
                redact_secrets: true,
            },
        }
    }
}

impl Config {
    pub fn load(project: &Project) -> Result<Self> {
        let config_path = project.cis_dir().join("config.toml");
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read {}", config_path.display()))?;
            let cfg: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse {}", config_path.display()))?;
            Ok(cfg)
        } else {
            let mut cfg = Config::default();
            cfg.general.project_name = project
                .root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            cfg.save(project)?;
            Ok(cfg)
        }
    }

    pub fn save(&self, project: &Project) -> Result<()> {
        let config_path = project.cis_dir().join("config.toml");
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;
        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write {}", config_path.display()))?;
        Ok(())
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() != 2 {
            anyhow::bail!("Config key must be in format <section>.<field>");
        }
        match parts[0] {
            "general" => match parts[1] {
                "project_name" => self.general.project_name = value.to_string(),
                "max_file_size_bytes" => {
                    self.general.max_file_size_bytes = value.parse()?
                }
                _ => anyhow::bail!("Unknown config key: {}", key),
            },
            "scanner" => match parts[1] {
                "enable_incremental" => {
                    self.scanner.enable_incremental = value.parse()?
                }
                _ => anyhow::bail!("Unknown config key: {}", key),
            },
            "database" => match parts[1] {
                "wal_mode" => self.database.wal_mode = value.parse()?,
                _ => anyhow::bail!("Unknown config key: {}", key),
            },
            "security" => match parts[1] {
                "default_timeout_seconds" => {
                    self.security.default_timeout_seconds = value.parse()?
                }
                "max_output_bytes" => {
                    self.security.max_output_bytes = value.parse()?
                }
                "require_confirmation_for_risky" => {
                    self.security.require_confirmation_for_risky = value.parse()?
                }
                "restrict_to_project_root" => {
                    self.security.restrict_to_project_root = value.parse()?
                }
                "redact_secrets" => {
                    self.security.redact_secrets = value.parse()?
                }
                _ => anyhow::bail!("Unknown config key: {}", key),
            },
            _ => anyhow::bail!("Unknown config section: {}", parts[0]),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_default_serialization() {
        let cfg = Config::default();
        let toml_str = toml::to_string(&cfg).unwrap();
        assert!(toml_str.contains("[general]"));
        assert!(toml_str.contains("[scanner]"));
        assert!(toml_str.contains("[database]"));
        assert!(toml_str.contains("[security]"));
    }

    #[test]
    fn test_config_set() {
        let mut cfg = Config::default();
        cfg.set("general.project_name", "MyProject").unwrap();
        assert_eq!(cfg.general.project_name, "MyProject");
    }

    #[test]
    fn test_config_set_invalid_key() {
        let mut cfg = Config::default();
        let result = cfg.set("invalid.key", "value");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_save_and_load() {
        let dir = tempdir().unwrap();
        let project = Project::new(dir.path().to_path_buf()).unwrap();
        project.init().unwrap();
        
        let mut cfg = Config::default();
        cfg.general.project_name = "TestProject".to_string();
        cfg.save(&project).unwrap();
        
        let loaded = Config::load(&project).unwrap();
        assert_eq!(loaded.general.project_name, "TestProject");
    }
}
