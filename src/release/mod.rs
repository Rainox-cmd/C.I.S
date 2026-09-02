use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::project::Project;

pub struct ReleaseManager;

impl ReleaseManager {
    pub fn version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
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
