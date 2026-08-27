use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write, Cursor};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::ZipArchive;
use zip::write::{ZipWriter, FileOptions};

use crate::index::Index;
use crate::memory::Provenance;
use crate::project::Project;

pub const EXPORT_FORMAT_VERSION: &str = "1.0";
const MANIFEST_NAME: &str = "manifest.json";
const PROJECT_MEMORY_DIR_NAME: &str = "context/project";
const SESSIONS_DIR_NAME: &str = "context/sessions";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFormat {
    pub version: String,
    pub created_at: u64,
    pub project_root: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    pub format: ExportFormat,
    pub files: Vec<ExportedFile>,
    pub sessions: Vec<ExportedSession>,
    pub project_memory: Vec<ExportedMemory>,
    pub db_backup_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedFile {
    pub rel_path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedSession {
    pub session_id: String,
    pub entries: Vec<ExportedMemory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedMemory {
    pub key: String,
    pub value: String,
    pub category: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    pub include_project_memory: bool,
    pub include_sessions: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            include_project_memory: true,
            include_sessions: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOptions {
    pub allow_overwrite: bool,
    pub skip_stale: bool,
    pub verify_hashes: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            allow_overwrite: false,
            skip_stale: false,
            verify_hashes: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub archive_path: String,
    pub files_exported: usize,
    pub sessions_exported: usize,
    pub project_memory_entries: usize,
    pub archive_size: u64,
    pub manifest: ExportManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub files_imported: usize,
    pub sessions_imported: usize,
    pub project_memory_entries: usize,
    pub stale_references: Vec<StaleReference>,
    pub changed_files: Vec<ChangedFile>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleReference {
    pub rel_path: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedFile {
    pub rel_path: String,
    pub status: String,
}

pub struct Exporter;

impl Exporter {
    pub fn export(
        project: &Project,
        idx: &Index,
        options: &ExportOptions,
        output_path: &Path,
    ) -> Result<ExportResult> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut files = Vec::new();
        let mut sessions = Vec::new();
        let mut project_memory = Vec::new();

        let db_backup_path = if project.db_path.exists() {
            let backup = format!("{}.cis_bak", project.db_path.to_string_lossy());
            let backup_path = PathBuf::from(&backup);
            fs::copy(&project.db_path, &backup_path)?;
            Some(backup_path.to_string_lossy().to_string())
        } else {
            None
        };

        if options.include_project_memory {
            project_memory = Self::collect_project_memory(project)?;
        }

        if options.include_sessions {
            sessions = Self::collect_sessions(project)?;
        }

        let file_hashes = idx.get_file_hashes()?;
        for (rel_path, sha256) in &file_hashes {
            let full_path = project.root.join(rel_path);
            let size = if full_path.exists() {
                fs::metadata(&full_path).map(|m| m.len()).unwrap_or(0)
            } else {
                0
            };
            files.push(ExportedFile {
                rel_path: rel_path.clone(),
                sha256: sha256.clone(),
                size,
            });
        }

        let manifest = ExportManifest {
            format: ExportFormat {
                version: EXPORT_FORMAT_VERSION.to_string(),
                created_at: now,
                project_root: project.root.to_string_lossy().to_string(),
                format: "zip".to_string(),
            },
            files,
            sessions: sessions.clone(),
            project_memory: project_memory.clone(),
            db_backup_path: db_backup_path.clone(),
        };

        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buffer);
            let option = FileOptions::default();

            zip.start_file(MANIFEST_NAME, option)?;
            zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;

            for mem in &project_memory {
                let path = format!("{}/{}.json", PROJECT_MEMORY_DIR_NAME, mem.key.replace('/', "__"));
                zip.start_file(&path, option)?;
                let entry_json = serde_json::to_string_pretty(&serde_json::json!({
                    "key": mem.key,
                    "value": mem.value,
                    "category": mem.category,
                    "tags": mem.tags,
                }))?;
                zip.write_all(entry_json.as_bytes())?;
            }

            for session in &sessions {
                for entry in &session.entries {
                    let path = format!(
                        "{}/session-{}/{}.json",
                        "context", SESSIONS_DIR_NAME.trim_start_matches("context/"),
                        session.session_id
                    );
                    let _ = path;
                    let path = format!(
                        "{}/session-{}/{}.json",
                        SESSIONS_DIR_NAME, session.session_id,
                        entry.key.replace('/', "__")
                    );
                zip.start_file(&path, option)?;
                let entry_json = serde_json::to_string_pretty(&serde_json::json!({
                    "key": entry.key,
                        "value": entry.value,
                        "category": entry.category,
                        "tags": entry.tags,
                    }))?;
                    zip.write_all(entry_json.as_bytes())?;
                }
            }

            if let Some(ref db_path) = db_backup_path {
                let db_data = fs::read(db_path)?;
                zip.start_file("project.db", option)?;
                zip.write_all(&db_data)?;
            }

            zip.finish()?;
        }

        let zip_data = buffer.into_inner();
        fs::write(output_path, &zip_data)
            .with_context(|| format!("Failed to write archive to {}", output_path.display()))?;

        let archive_size = zip_data.len() as u64;

        if let Some(ref db_path) = db_backup_path {
            let _ = fs::remove_file(db_path);
        }

        Ok(ExportResult {
            archive_path: output_path.to_string_lossy().to_string(),
            files_exported: manifest.files.len(),
            sessions_exported: sessions.len(),
            project_memory_entries: project_memory.len(),
            archive_size,
            manifest: ExportManifest {
                format: manifest.format,
                files: vec![],
                sessions: vec![],
                project_memory: vec![],
                db_backup_path: manifest.db_backup_path,
            },
        })
    }

    fn collect_project_memory(project: &Project) -> Result<Vec<ExportedMemory>> {
        let mut result = Vec::new();
        if !project.project_memory_dir.exists() {
            return Ok(result);
        }
        for entry in fs::read_dir(&project.project_memory_dir)? {
            let entry = entry?;
            if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                let content = fs::read_to_string(entry.path())?;
                let mem: serde_json::Value = serde_json::from_str(&content)?;
                if let Some(key) = mem.get("key").and_then(|v| v.as_str()) {
                    result.push(ExportedMemory {
                        key: key.to_string(),
                        value: mem.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        category: mem.get("category").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        tags: mem.get("tags")
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                            .unwrap_or_default(),
                    });
                }
            }
        }
        Ok(result)
    }

    fn collect_sessions(project: &Project) -> Result<Vec<ExportedSession>> {
        let mut result = Vec::new();
        if !project.sessions_dir.exists() {
            return Ok(result);
        }
        for entry in fs::read_dir(&project.sessions_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    if let Some(session_id) = name.strip_prefix("session-") {
                        let session_dir = entry.path();
                        let mut entries = Vec::new();
                        for file_entry in fs::read_dir(&session_dir)? {
                            let file_entry = file_entry?;
                            if file_entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                                let content = fs::read_to_string(file_entry.path())?;
                                let mem: serde_json::Value = serde_json::from_str(&content)?;
                                entries.push(ExportedMemory {
                                    key: mem.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    value: mem.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    category: mem.get("category").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    tags: mem.get("tags")
                                        .and_then(|v| v.as_array())
                                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                                        .unwrap_or_default(),
                                });
                            }
                        }
                        result.push(ExportedSession {
                            session_id: session_id.to_string(),
                            entries,
                        });
                    }
                }
            }
        }
        Ok(result)
    }
}

pub struct Importer;

impl Importer {
    pub fn import(
        archive_path: &Path,
        project: &Project,
        options: &ImportOptions,
    ) -> Result<ImportResult> {
        let file = fs::File::open(archive_path)
            .with_context(|| format!("Failed to open archive at {}", archive_path.display()))?;
        let mut archive = ZipArchive::new(file)?;

        let mut manifest_buf = Vec::new();
        let mut has_manifest = false;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if entry.name() == MANIFEST_NAME {
                entry.read_to_end(&mut manifest_buf)?;
                has_manifest = true;
                break;
            }
        }

        if !has_manifest {
            anyhow::bail!("Archive does not contain a valid manifest");
        }

        let manifest: ExportManifest = serde_json::from_slice(&manifest_buf)
            .context("Failed to parse manifest from archive")?;

        if manifest.format.version != EXPORT_FORMAT_VERSION {
            anyhow::bail!(
                "Format version mismatch: expected {}, got {}",
                EXPORT_FORMAT_VERSION,
                manifest.format.version
            );
        }

        let mut result = ImportResult {
            files_imported: 0,
            sessions_imported: 0,
            project_memory_entries: 0,
            stale_references: Vec::new(),
            changed_files: Vec::new(),
            warnings: Vec::new(),
        };

        let project_mem_exists = project.project_memory_dir.exists()
            && project.project_memory_dir.read_dir()?.next().is_some();

        if project_mem_exists && !options.allow_overwrite {
            result.warnings.push(
                "Project memory directory is not empty. Existing entries preserved. Use --allow-overwrite to replace.".to_string()
            );
        }

        for mem in &manifest.project_memory {
            let mem_path = project.project_memory_dir.join(format!("{}.json", mem.key.replace('/', "__")));
            if mem_path.exists() && !options.allow_overwrite {
                result.warnings.push(format!("Skipping existing entry (use --allow-overwrite): {}", mem.key));
                continue;
            }

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let mem_entry = crate::memory::MemoryEntry {
                key: mem.key.clone(),
                value: mem.value.clone(),
                provenance: Provenance::Inferred,
                created_at: now,
                updated_at: now,
                expires_at: None,
                tags: mem.tags.clone(),
                category: mem.category.clone(),
            };
            let entry_json = serde_json::to_string_pretty(&mem_entry)?;
            fs::write(&mem_path, entry_json)?;
            result.project_memory_entries += 1;
        }

        for session in &manifest.sessions {
            let session_dir = project.sessions_dir.join(format!("session-{}", session.session_id));

            if session_dir.exists() && !options.allow_overwrite {
                result.warnings.push(format!("Session '{}' exists and is not empty. Skipping.", session.session_id));
                continue;
            }

            if !session_dir.exists() {
                fs::create_dir_all(&session_dir)?;
            }

            for entry in &session.entries {
                let entry_path = session_dir.join(format!("{}.json", entry.key.replace('/', "__")));
                if entry_path.exists() && !options.allow_overwrite {
                    result.warnings.push(format!("Skipping existing session entry (use --allow-overwrite): {}", entry.key));
                    continue;
                }
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let mem_entry = crate::memory::MemoryEntry {
                    key: entry.key.clone(),
                    value: entry.value.clone(),
                    provenance: Provenance::Inferred,
                    created_at: now,
                    updated_at: now,
                    expires_at: None,
                    tags: entry.tags.clone(),
                    category: entry.category.clone(),
                };
                let entry_json = serde_json::to_string_pretty(&mem_entry)?;
                fs::write(&entry_path, entry_json)?;
            }
            result.sessions_imported += 1;
        }

        result.files_imported = manifest.files.len();

        if options.verify_hashes {
            for f in &manifest.files {
                let full_path = project.root.join(&f.rel_path);
                if full_path.exists() {
                    let content = fs::read(&full_path)?;
                    let mut hasher = Sha256::new();
                    hasher.update(&content);
                    let hash = format!("{:x}", hasher.finalize());
                    if hash != f.sha256 {
                        result.changed_files.push(ChangedFile {
                            rel_path: f.rel_path.clone(),
                            status: "hash_mismatch".to_string(),
                        });
                    }
                } else {
                    result.changed_files.push(ChangedFile {
                        rel_path: f.rel_path.clone(),
                        status: "file_missing".to_string(),
                    });
                }
            }
        }

        Ok(result)
    }

    pub fn verify_archive(archive_path: &Path) -> Result<bool> {
        let file = fs::File::open(archive_path)?;
        let mut archive = ZipArchive::new(file)?;

        let mut has_manifest = false;
        for i in 0..archive.len() {
            let entry = archive.by_index(i)?;
            if entry.name() == MANIFEST_NAME {
                has_manifest = true;
                break;
            }
        }

        Ok(has_manifest)
    }

    pub fn compute_file_hash(path: &Path) -> Result<String> {
        let data = fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    pub fn detect_changes(
        _project: &Project,
        idx: &Index,
        imported_files: &[ExportedFile],
    ) -> Result<Vec<ChangedFile>> {
        let db_hashes = idx.get_file_hashes()?;
        let mut changed = Vec::new();

        for f in imported_files {
            let rel_path = &f.rel_path;
            match db_hashes.get(rel_path) {
                Some(db_hash) => {
                    if db_hash != &f.sha256 {
                        changed.push(ChangedFile {
                            rel_path: rel_path.clone(),
                            status: "modified".to_string(),
                        });
                    }
                }
                None => {
                    changed.push(ChangedFile {
                        rel_path: rel_path.clone(),
                        status: "missing".to_string(),
                    });
                }
            }
        }

        Ok(changed)
    }
}
