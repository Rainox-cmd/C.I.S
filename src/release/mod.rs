use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::project::Project;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseConfig {
    pub version: String,
    pub version_major: u32,
    pub version_minor: u32,
    pub version_patch: u32,
    pub targets: Vec<ReleaseTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseTarget {
    pub platform: String,
    pub arch: String,
    pub extension: String,
    pub package_format: String,
}

impl Default for ReleaseConfig {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            version_major: 1,
            version_minor: 0,
            version_patch: 0,
            targets: vec![
                ReleaseTarget {
                    platform: "windows".to_string(),
                    arch: "x86_64".to_string(),
                    extension: "exe".to_string(),
                    package_format: "msi".to_string(),
                },
                ReleaseTarget {
                    platform: "linux".to_string(),
                    arch: "x86_64".to_string(),
                    extension: "".to_string(),
                    package_format: "deb".to_string(),
                },
                ReleaseTarget {
                    platform: "linux".to_string(),
                    arch: "x86_64".to_string(),
                    extension: "".to_string(),
                    package_format: "rpm".to_string(),
                },
                ReleaseTarget {
                    platform: "macos".to_string(),
                    arch: "x86_64".to_string(),
                    extension: "".to_string(),
                    package_format: "dmg".to_string(),
                },
                ReleaseTarget {
                    platform: "macos".to_string(),
                    arch: "aarch64".to_string(),
                    extension: "".to_string(),
                    package_format: "dmg".to_string(),
                },
            ],
        }
    }
}

pub struct ReleaseManager;

impl ReleaseManager {
    pub fn version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    pub fn version_parts() -> (u32, u32, u32) {
        let parts: Vec<&str> = env!("CARGO_PKG_VERSION").split('.').collect();
        let major = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
        let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    }

    pub fn build_release_name(platform: &str, arch: &str, package_format: &str) -> String {
        format!("cis-{}-{}.{}", platform, arch, package_format)
    }

    pub fn changelog_path(project: &Project) -> PathBuf {
        project.root.join("CHANGELOG.md")
    }

    pub fn generate_changelog(project: &Project) -> Result<String> {
        let git_log = Self::git_log(project)?;
        let changelog = format!(
            "# Changelog\n\n## v{}\n\n### Changes\n{}\n",
            Self::version(),
            git_log
        );
        Ok(changelog)
    }

    fn git_log(project: &Project) -> Result<String> {
        let client = crate::git::GitClient::new(&project.root)?;
        if !client.is_repo() {
            return Ok(String::new());
        }
        let commits = client.log(10)?;
        let mut log_str = String::new();
        for commit in commits {
            log_str.push_str(&format!("{} {}\n", commit.short_hash, commit.message));
        }
        Ok(log_str)
    }

    pub fn save_changelog(project: &Project) -> Result<()> {
        let changelog = Self::generate_changelog(project)?;
        let path = Self::changelog_path(project);
        fs::write(&path, changelog)
            .with_context(|| format!("Failed to write changelog to {}", path.display()))?;
        Ok(())
    }

    pub fn validate_build(project: &Project) -> Result<bool> {
        let binary_path = project.root.join("target").join("release").join("cis");
        #[cfg(windows)]
        let binary_path = binary_path.with_extension("exe");
        Ok(binary_path.exists())
    }
}

#[cfg(test)]
mod tests;
