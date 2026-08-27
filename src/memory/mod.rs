use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::ContextConfig;
use crate::project::Project;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provenance {
    Detected,
    Inferred,
    AiGenerated,
    DeveloperConfirmed,
}

impl std::fmt::Display for Provenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provenance::Detected => write!(f, "detected"),
            Provenance::Inferred => write!(f, "inferred"),
            Provenance::AiGenerated => write!(f, "ai_generated"),
            Provenance::DeveloperConfirmed => write!(f, "developer_confirmed"),
        }
    }
}

impl std::str::FromStr for Provenance {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "detected" => Ok(Provenance::Detected),
            "inferred" => Ok(Provenance::Inferred),
            "ai_generated" => Ok(Provenance::AiGenerated),
            "developer_confirmed" => Ok(Provenance::DeveloperConfirmed),
            _ => anyhow::bail!("Unknown provenance: {}", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    pub provenance: Provenance,
    pub created_at: u64,
    pub updated_at: u64,
    pub expires_at: Option<u64>,
    pub tags: Vec<String>,
    pub category: String,
}

impl MemoryEntry {
    pub fn new(
        key: String,
        value: String,
        provenance: Provenance,
        category: String,
        ttl_seconds: Option<u64>,
        tags: Vec<String>,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires_at = ttl_seconds.map(|ttl| now + ttl);
        Self {
            key,
            value,
            provenance,
            created_at: now,
            updated_at: now,
            expires_at,
            tags,
            category,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            now > expires
        } else {
            false
        }
    }
}

pub struct MemoryManager {
    project: Project,
    config: ContextConfig,
}

impl MemoryManager {
    pub fn new(project: &Project, config: &ContextConfig) -> Result<Self> {
        fs::create_dir_all(&project.project_memory_dir)
            .with_context(|| format!("Failed to create {}", project.project_memory_dir.display()))?;
        fs::create_dir_all(&project.sessions_dir)
            .with_context(|| format!("Failed to create {}", project.sessions_dir.display()))?;
        Ok(Self {
            project: project.clone(),
            config: config.clone(),
        })
    }

    fn project_entry_path(key: &str) -> String {
         let safe_key = key.replace(['/', '\\'], "__");
        format!("{}.json", safe_key)
    }

    fn session_dir(session_id: &str) -> String {
        format!("session-{}", session_id)
    }

    pub fn project_set(
        &self,
        key: &str,
        value: &str,
        provenance: Provenance,
        category: &str,
        ttl_seconds: Option<u64>,
        tags: Vec<String>,
    ) -> Result<()> {
        self.check_project_context_budget(value.len() as u64)?;

        let entry = MemoryEntry::new(
            key.to_string(),
            value.to_string(),
            provenance,
            category.to_string(),
            ttl_seconds,
            tags,
        );

        let entry_path = self.project.project_memory_dir.join(Self::project_entry_path(key));

        if entry_path.exists() {
            let existing: MemoryEntry = serde_json::from_str(
                &fs::read_to_string(&entry_path)
                    .with_context(|| format!("Failed to read {}", entry_path.display()))?,
            )
            .context("Failed to parse existing memory entry")?;
            let mut entry = entry;
            entry.created_at = existing.created_at;
            fs::write(&entry_path, serde_json::to_string_pretty(&entry)?)
                .with_context(|| format!("Failed to write {}", entry_path.display()))?;
        } else {
            fs::write(&entry_path, serde_json::to_string_pretty(&entry)?)
                .with_context(|| format!("Failed to write {}", entry_path.display()))?;
        }

        Ok(())
    }

    pub fn project_get(&self, key: &str) -> Result<Option<MemoryEntry>> {
        let entry_path = self.project.project_memory_dir.join(Self::project_entry_path(key));
        if !entry_path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&entry_path)
            .with_context(|| format!("Failed to read {}", entry_path.display()))?;
        let entry: MemoryEntry = serde_json::from_str(&content).context("Failed to parse memory entry")?;
        if entry.is_expired() {
            let _ = fs::remove_file(&entry_path);
            return Ok(None);
        }
        Ok(Some(entry))
    }

    pub fn project_list(&self) -> Result<Vec<MemoryEntry>> {
        let mut entries = Vec::new();
        if !self.project.project_memory_dir.exists() {
            return Ok(entries);
        }
        for entry in fs::read_dir(&self.project.project_memory_dir)
            .with_context(|| format!("Failed to read {}", self.project.project_memory_dir.display()))?
        {
            let entry = entry?;
            if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                let content = fs::read_to_string(entry.path())?;
                let entry: MemoryEntry = serde_json::from_str(&content)?;
                if !entry.is_expired() {
                    entries.push(entry);
                }
            }
        }
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(entries)
    }

    pub fn project_delete(&self, key: &str) -> Result<bool> {
        let entry_path = self.project.project_memory_dir.join(Self::project_entry_path(key));
        if entry_path.exists() {
            fs::remove_file(&entry_path)?;
            return Ok(true);
        }
        Ok(false)
    }

    #[allow(dead_code)]
    pub fn project_list_keys(&self) -> Result<Vec<String>> {
        let entries = self.project_list()?;
        Ok(entries.into_iter().map(|e| e.key).collect())
    }

    pub fn session_set(
        &self,
        session_id: &str,
        key: &str,
        value: &str,
        provenance: Provenance,
        ttl_seconds: Option<u64>,
        tags: Vec<String>,
    ) -> Result<()> {
        let session_dir = self.project.sessions_dir.join(Self::session_dir(session_id));
        self.check_session_budget(session_id, value.len() as u64)?;
        self.check_total_storage_limit()?;

        fs::create_dir_all(&session_dir)
            .with_context(|| format!("Failed to create session dir {}", session_dir.display()))?;

        let entry = MemoryEntry::new(
            key.to_string(),
            value.to_string(),
            provenance,
            "session".to_string(),
            ttl_seconds,
            tags,
        );

        let entry_path = session_dir.join(Self::project_entry_path(key));
        fs::write(&entry_path, serde_json::to_string_pretty(&entry)?)
            .with_context(|| format!("Failed to write {}", entry_path.display()))?;

        Ok(())
    }

    pub fn session_get(&self, session_id: &str, key: &str) -> Result<Option<MemoryEntry>> {
        let session_dir = self.project.sessions_dir.join(Self::session_dir(session_id));
        let entry_path = session_dir.join(Self::project_entry_path(key));
        if !entry_path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&entry_path)?;
        let entry: MemoryEntry = serde_json::from_str(&content)?;
        if entry.is_expired() {
            let _ = fs::remove_file(&entry_path);
            return Ok(None);
        }
        Ok(Some(entry))
    }

    pub fn session_list(&self, session_id: &str) -> Result<Vec<MemoryEntry>> {
        let session_dir = self.project.sessions_dir.join(Self::session_dir(session_id));
        let mut entries = Vec::new();
        if !session_dir.exists() {
            return Ok(entries);
        }
        for entry in fs::read_dir(&session_dir)? {
            let entry = entry?;
            if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                let content = fs::read_to_string(entry.path())?;
                let entry: MemoryEntry = serde_json::from_str(&content)?;
                if !entry.is_expired() {
                    entries.push(entry);
                }
            }
        }
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(entries)
    }

    pub fn session_delete(&self, session_id: &str, key: &str) -> Result<bool> {
        let session_dir = self.project.sessions_dir.join(Self::session_dir(session_id));
        let entry_path = session_dir.join(Self::project_entry_path(key));
        if entry_path.exists() {
            fs::remove_file(&entry_path)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn list_sessions(&self) -> Result<Vec<String>> {
        if !self.project.sessions_dir.exists() {
            return Ok(vec![]);
        }
        let mut sessions = Vec::new();
        for entry in fs::read_dir(&self.project.sessions_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    if let Some(id) = name.strip_prefix("session-") {
                        sessions.push(id.to_string());
                    }
                }
            }
        }
        sessions.sort();
        Ok(sessions)
    }

    pub fn session_count(&self) -> Result<usize> {
        Ok(self.list_sessions()?.len())
    }

    fn check_project_context_budget(&self, new_entry_size: u64) -> Result<()> {
        if self.project_context_size()? + new_entry_size > self.config.max_session_context_bytes {
            anyhow::bail!(
                "Project memory context limit exceeded: max {} bytes",
                self.config.max_session_context_bytes
            );
        }
        Ok(())
    }

    fn check_session_budget(&self, session_id: &str, new_entry_size: u64) -> Result<()> {
        let session_dir = self.project.sessions_dir.join(Self::session_dir(session_id));
        let session_size = self.dir_size(&session_dir)?;
        if session_size + new_entry_size > self.config.max_session_context_bytes {
            anyhow::bail!(
                "Session context limit exceeded: max {} bytes for session {}",
                self.config.max_session_context_bytes,
                session_id
            );
        }
        Ok(())
    }

    fn check_total_storage_limit(&self) -> Result<()> {
        let total = self.context_storage_size()?;
        if total > self.config.max_total_context_bytes {
            anyhow::bail!(
                "Total context storage limit exceeded: max {} bytes",
                self.config.max_total_context_bytes
            );
        }
        Ok(())
    }

    fn dir_size(&self, dir: &std::path::Path) -> Result<u64> {
        if !dir.exists() {
            return Ok(0);
        }
        let mut total: u64 = 0;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                total += entry.metadata()?.len();
            } else if path.is_dir() {
                total += self.dir_size(&path)?;
            }
        }
        Ok(total)
    }

    fn project_context_size(&self) -> Result<u64> {
        self.dir_size(&self.project.project_memory_dir)
    }

    fn context_storage_size(&self) -> Result<u64> {
        let total = self.dir_size(&self.project.context_dir)?;
        Ok(total)
    }

    pub fn expired_sessions(&self) -> Result<Vec<String>> {
        if !self.project.sessions_dir.exists() {
            return Ok(vec![]);
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut expired = Vec::new();
        for entry in fs::read_dir(&self.project.sessions_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    if let Some(session_id) = name.strip_prefix("session-") {
                        let session_dir = self.project.sessions_dir.join(name);
                        if self.is_session_expired(&session_dir, now)? {
                            expired.push(session_id.to_string());
                        }
                    }
                }
            }
        }
        Ok(expired)
    }

    fn is_session_expired(&self, session_dir: &PathBuf, now: u64) -> Result<bool> {
        let ttl = self.config.session_ttl_seconds;
        let mut found_any = false;
        for entry in fs::read_dir(session_dir)? {
            let entry = entry?;
            if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                let content = fs::read_to_string(entry.path())?;
                let entry: MemoryEntry = serde_json::from_str(&content)?;
                found_any = true;
                if let Some(expires) = entry.expires_at {
                    if now > expires {
                        return Ok(true);
                    }
                }
                let entry_age = now.saturating_sub(entry.created_at);
                if entry_age > ttl {
                    return Ok(true);
                }
            }
        }
        if !found_any {
            return Ok(true);
        }
        Ok(false)
    }

    pub fn prune_expired_sessions(&self) -> Result<Vec<String>> {
        let expired = self.expired_sessions()?;
        let mut pruned = Vec::new();
        for session_id in &expired {
            let session_dir = self.project.sessions_dir.join(Self::session_dir(session_id));
            if self.export_before_delete(&session_dir, session_id)? {
                pruned.push(session_id.clone());
            }
        }
        Ok(pruned)
    }

    fn export_before_delete(&self, session_dir: &PathBuf, session_id: &str) -> Result<bool> {
        let export_dir = self.project.cache_dir.join("deleted_sessions");
        fs::create_dir_all(&export_dir)?;
        let archive_path = export_dir.join(format!("session-{}.tar", session_id));
        let entries = self.session_list(session_id)?;
        let archive_content = serde_json::to_string_pretty(&entries)
            .context("Failed to serialize session for archive")?;
        fs::write(&archive_path, archive_content)?;
        fs::remove_dir_all(session_dir)?;
        Ok(true)
    }

    pub fn get_storage_usage(&self) -> Result<(u64, u64, usize)> {
        let project_size = self.project_context_size()?;
        let context_size = self.context_storage_size()?;
        let session_count = self.session_count()?;
        Ok((project_size, context_size, session_count))
    }

    #[allow(dead_code)]
    pub fn check_limits(&self) -> Result<()> {
        let (project_size, _total_size, session_count) = self.get_storage_usage()?;

        if project_size > self.config.max_session_context_bytes {
            anyhow::bail!(
                "Project memory exceeds per-session limit: {} > {}",
                project_size,
                self.config.max_session_context_bytes
            );
        }
        if session_count > self.config.max_sessions {
            anyhow::bail!(
                "Number of sessions exceeds limit: {} > {}",
                session_count,
                self.config.max_sessions
            );
        }
        Ok(())
    }

    pub fn warn_if_near_limits(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if let Ok((project_size, _total_size, session_count)) = self.get_storage_usage() {
            let session_pct = (project_size as f64 / self.config.max_session_context_bytes as f64) * 100.0;
            if session_pct > 80.0 {
                warnings.push(format!(
                    "Project memory is {:.0}% of the per-session limit ({} / {} bytes)",
                    session_pct, project_size, self.config.max_session_context_bytes
                ));
            }
            let session_pct = (session_count as f64 / self.config.max_sessions as f64) * 100.0;
            if session_pct > 80.0 {
                warnings.push(format!(
                    "Session count is {:.0}% of the limit ({}/{})",
                    session_pct, session_count, self.config.max_sessions
                ));
            }
        }
        warnings
    }
}

#[cfg(test)]
mod tests;
