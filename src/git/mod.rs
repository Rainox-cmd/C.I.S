use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct GitStatusEntry {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct GitDiffEntry {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub additions: i64,
    pub deletions: i64,
}

#[derive(Debug, Clone)]
pub struct GitCommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub author_email: String,
    pub date: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct GitFileHistory {
    pub commits: Vec<GitCommitInfo>,
}

pub struct GitClient {
    pub repo_path: PathBuf,
}

impl GitClient {
    pub fn new(repo_path: &Path) -> Result<Self> {
        let canonical = std::fs::canonicalize(repo_path)
            .with_context(|| format!("Failed to canonicalize path: {}", repo_path.display()))?;
        Ok(Self { repo_path: canonical })
    }

    pub fn is_repo(&self) -> bool {
        let result = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(&self.repo_path)
            .output();
        match result {
            Ok(output) => output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true",
            Err(_) => false,
        }
    }

    pub fn current_branch(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["symbolic-ref", "--short", "HEAD"])
            .current_dir(&self.repo_path)
            .output()
            .context("Failed to execute git symbolic-ref")?;
        if !output.status.success() {
            let rev_output = Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .current_dir(&self.repo_path)
                .output()
                .context("Failed to execute git rev-parse")?;
            if !rev_output.status.success() {
                anyhow::bail!("git branch detection failed: {}", String::from_utf8_lossy(&rev_output.stderr));
            }
            return Ok(String::from_utf8_lossy(&rev_output.stdout).trim().to_string());
        }
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch.is_empty() {
            let rev_output = Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .current_dir(&self.repo_path)
                .output()
                .context("Failed to execute git rev-parse")?;
            return Ok(String::from_utf8_lossy(&rev_output.stdout).trim().to_string());
        }
        Ok(branch)
    }

    pub fn head_commit_hash(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.repo_path)
            .output()
            .context("Failed to execute git rev-parse HEAD")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("does not have any commits") || stderr.contains("unknown revision") {
                return Ok(String::new());
            }
            anyhow::bail!("git rev-parse HEAD failed: {}", stderr);
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn short_commit_hash(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(&self.repo_path)
            .output()
            .context("Failed to execute git rev-parse --short HEAD")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("does not have any commits") || stderr.contains("unknown revision") {
                return Ok(String::new());
            }
            anyhow::bail!("git rev-parse --short HEAD failed: {}", stderr);
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn status(&self) -> Result<Vec<GitStatusEntry>> {
        let output = Command::new("git")
            .args(["status", "--porcelain", "-z"])
            .current_dir(&self.repo_path)
            .output()
            .context("Failed to execute git status")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("does not have any commits") || stderr.contains("unknown revision") {
                return Ok(vec![]);
            }
            anyhow::bail!("git status failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let entries: Vec<GitStatusEntry> = stdout
            .split('\0')
            .filter(|s| !s.is_empty())
            .filter_map(|entry| {
                if entry.len() < 3 {
                    return None;
                }
                let status = entry.chars().take(2).collect::<String>();
                let path = entry[3..].to_string();
                let path = path.split('\0').next().unwrap_or(&path).to_string();
                if status.trim().is_empty() {
                    None
                } else {
                    Some(GitStatusEntry {
                        path,
                        status: status.trim().to_string(),
                    })
                }
            })
            .collect();

        Ok(entries)
    }

    pub fn is_dirty(&self) -> Result<bool> {
        let output = Command::new("git")
            .args(["status", "--porcelain", "-z"])
            .current_dir(&self.repo_path)
            .output()
            .context("Failed to execute git status")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("does not have any commits") || stderr.contains("unknown revision") {
                return Ok(false);
            }
            anyhow::bail!("git status failed: {}", stderr);
        }
        Ok(!String::from_utf8_lossy(&output.stdout).is_empty())
    }

    pub fn diff(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["diff"])
            .current_dir(&self.repo_path)
            .output()
            .context("Failed to execute git diff")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("does not have any commits") || stderr.contains("unknown revision") {
                return Ok(String::new());
            }
            anyhow::bail!("git diff failed: {}", stderr);
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn diff_staged(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["diff", "--cached"])
            .current_dir(&self.repo_path)
            .output()
            .context("Failed to execute git diff --cached")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("does not have any commits") || stderr.contains("unknown revision") {
                return Ok(String::new());
            }
            anyhow::bail!("git diff --cached failed: {}", stderr);
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn diff_stat(&self) -> Result<Vec<GitDiffEntry>> {
        let output = Command::new("git")
            .args(["diff", "--numstat"])
            .current_dir(&self.repo_path)
            .output()
            .context("Failed to execute git diff --numstat")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("does not have any commits") || stderr.contains("unknown revision") {
                return Ok(vec![]);
            }
            anyhow::bail!("git diff --numstat failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let entries: Vec<GitDiffEntry> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() < 3 {
                    return None;
                }
                let additions = parts[0].parse::<i64>().unwrap_or(0);
                let deletions = parts[1].parse::<i64>().unwrap_or(0);
                let path = parts[2].to_string();
                let old_path = if parts.len() > 3 && parts[2] != parts[3] {
                    Some(parts[3].to_string())
                } else {
                    None
                };
                let status = if additions > 0 && deletions > 0 {
                    "modified"
                } else if additions > 0 {
                    "added"
                } else if deletions > 0 {
                    "deleted"
                } else {
                    "unmerged"
                };
                Some(GitDiffEntry {
                    path,
                    old_path,
                    status: status.to_string(),
                    additions,
                    deletions,
                })
            })
            .collect();

        Ok(entries)
    }

    pub fn log(&self, count: usize) -> Result<Vec<GitCommitInfo>> {
        let format = "%H%x1f%h%x1f%an%x1f%ae%x1f%ad%x1f%s";
        let output = Command::new("git")
            .args(["log", &format!("--max-count={}", count), &format!("--format={}", format)])
            .current_dir(&self.repo_path)
            .output()
            .context("Failed to execute git log")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("does not have any commits") || stderr.contains("unknown revision") {
                return Ok(vec![]);
            }
            anyhow::bail!("git log failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let commits: Vec<GitCommitInfo> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\x1f').collect();
                if parts.len() < 6 {
                    return None;
                }
                Some(GitCommitInfo {
                    hash: parts[0].to_string(),
                    short_hash: parts[1].to_string(),
                    author: parts[2].to_string(),
                    author_email: parts[3].to_string(),
                    date: parts[4].to_string(),
                    message: parts[5].to_string(),
                })
            })
            .collect();

        Ok(commits)
    }

    pub fn file_history(&self, file_path: &str, limit: usize) -> Result<Vec<GitCommitInfo>> {
        let format = "%H%x1f%h%x1f%an%x1f%ae%x1f%ad%x1f%s";
        let output = Command::new("git")
            .args([
                "log",
                &format!("--max-count={}", limit),
                &format!("--format={}", format),
                "--",
                file_path,
            ])
            .current_dir(&self.repo_path)
            .output()
            .with_context(|| format!("Failed to execute git log for {}", file_path))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("does not have any commits") || stderr.contains("unknown revision") || stderr.contains("no such path") {
                return Ok(vec![]);
            }
            anyhow::bail!("git log for {} failed: {}", file_path, stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let commits: Vec<GitCommitInfo> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\x1f').collect();
                if parts.len() < 6 {
                    return None;
                }
                Some(GitCommitInfo {
                    hash: parts[0].to_string(),
                    short_hash: parts[1].to_string(),
                    author: parts[2].to_string(),
                    author_email: parts[3].to_string(),
                    date: parts[4].to_string(),
                    message: parts[5].to_string(),
                })
            })
            .collect();

        Ok(commits)
    }

    pub fn file_last_commit(&self, file_path: &str) -> Result<Option<GitCommitInfo>> {
        let commits = self.file_history(file_path, 1)?;
        Ok(commits.into_iter().next())
    }

    pub fn link_file_commit(&self, rel_path: &str) -> Result<Option<String>> {
        let commits = self.file_history(rel_path, 1)?;
        Ok(commits.into_iter().next().map(|c| c.short_hash))
    }

    pub fn recent_changes(&self, count: usize) -> Result<Vec<GitCommitInfo>> {
        self.log(count)
    }
}

#[cfg(test)]
mod tests;
