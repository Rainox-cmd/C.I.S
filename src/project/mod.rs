use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const PROJECT_MARKERS: [&str; 4] = [".git", "Cargo.toml", "package.json", "pyproject.toml"];

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub cis_dir: PathBuf,
    pub db_path: PathBuf,
    pub logs_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub context_dir: PathBuf,
    pub project_memory_dir: PathBuf,
    pub sessions_dir: PathBuf,
}

impl Project {
    pub fn discover() -> Result<Self> {
        let cwd = std::env::current_dir().context("Failed to get current directory")?;
        let root = find_project_root(&cwd)
            .ok_or_else(|| anyhow::anyhow!("Could not find project root. Run from within a project directory containing .git, Cargo.toml, package.json, or pyproject.toml."))?;
        Self::new(root)
    }

    pub fn new(root: PathBuf) -> Result<Self> {
        let cis_dir = root.join(".cis");
        let db_path = cis_dir.join("project.db");
        let logs_dir = cis_dir.join("logs");
        let cache_dir = cis_dir.join("cache");
        let context_dir = cis_dir.join("context");
        let project_memory_dir = context_dir.join("project");
        let sessions_dir = context_dir.join("sessions");
        Ok(Self {
            root,
            cis_dir,
            db_path,
            logs_dir,
            cache_dir,
            context_dir,
            project_memory_dir,
            sessions_dir,
        })
    }

    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.cis_dir)
            .with_context(|| format!("Failed to create {}", self.cis_dir.display()))?;
        fs::create_dir_all(&self.logs_dir)
            .with_context(|| format!("Failed to create {}", self.logs_dir.display()))?;
        fs::create_dir_all(&self.cache_dir)
            .with_context(|| format!("Failed to create {}", self.cache_dir.display()))?;
        fs::create_dir_all(&self.project_memory_dir)
            .with_context(|| format!("Failed to create {}", self.project_memory_dir.display()))?;
        fs::create_dir_all(&self.sessions_dir)
            .with_context(|| format!("Failed to create {}", self.sessions_dir.display()))?;
        Ok(())
    }

    pub fn cis_dir(&self) -> &Path {
        &self.cis_dir
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        for marker in &PROJECT_MARKERS {
            if current.join(marker).exists() {
                return Some(current);
            }
        }
        if !current.pop() {
            break;
        }
    }
    None
}
