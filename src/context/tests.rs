use super::export::*;
use crate::config::Config;
use crate::index::Index;
use crate::project::Project;
use std::fs;
use std::io::Read;
use tempfile::tempdir;
use zip::ZipArchive;
use zip::ZipWriter;

fn setup() -> (tempfile::TempDir, Project, Config) {
    let dir = tempdir().unwrap();
    let project = Project::new(dir.path().to_path_buf()).unwrap();
    project.init().unwrap();
    let cfg = Config::default();
    (dir, project, cfg)
}

#[test]
fn test_export_creates_valid_archive() {
    let (dir, project, cfg) = setup();
    let idx = Index::open(&project, &cfg).unwrap();
    let archive_path = dir.path().join("context.zip");

    let options = ExportOptions::default();
    let result = Exporter::export(&project, &idx, &options, &archive_path).unwrap();

    assert_eq!(result.archive_path, archive_path.to_string_lossy().to_string());
    assert!(archive_path.exists());
    assert!(Importer::verify_archive(&archive_path).unwrap());
}

#[test]
fn test_import_restores_context() {
    let (dir, project, cfg) = setup();
    let idx = Index::open(&project, &cfg).unwrap();

    let mem = crate::memory::MemoryManager::new(&project, &cfg.context).unwrap();
    mem.project_set("test_key", "test_value", crate::memory::Provenance::DeveloperConfirmed, "test", None, vec!["tag1".into()]).unwrap();
    mem.session_set("sess1", "task", "working", crate::memory::Provenance::Detected, None, vec![]).unwrap();

    let archive_path = dir.path().join("context.zip");
    let options = ExportOptions::default();
    Exporter::export(&project, &idx, &options, &archive_path).unwrap();

    let (_dir2, project2, cfg2) = setup();
    let _idx2 = Index::open(&project2, &cfg2).unwrap();

    let import_opts = ImportOptions::default();
    let result = Importer::import(&archive_path, &project2, &import_opts).unwrap();

    assert_eq!(result.project_memory_entries, 1);
    assert_eq!(result.sessions_imported, 1);

    let mem2 = crate::memory::MemoryManager::new(&project2, &cfg2.context).unwrap();
    let entry = mem2.project_get("test_key").unwrap();
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().value, "test_value");

    let entry = mem2.session_get("sess1", "task").unwrap();
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().value, "working");
}

#[test]
fn test_stale_references_detected() {
    let (dir, project, cfg) = setup();
    let idx = Index::open(&project, &cfg).unwrap();

    let archive_path = dir.path().join("context.zip");
    let options = ExportOptions::default();
    Exporter::export(&project, &idx, &options, &archive_path).unwrap();

    let (_dir2, project2, cfg2) = setup();
    let _idx2 = Index::open(&project2, &cfg2).unwrap();

    let import_opts = ImportOptions::default();
    let result = Importer::import(&archive_path, &project2, &import_opts).unwrap();

    for r in &result.stale_references {
        assert!(!r.rel_path.is_empty());
    }
}

#[test]
fn test_changed_files_flagged() {
    let (dir, project, cfg) = setup();
    let idx = Index::open(&project, &cfg).unwrap();

    // Create a source file and index it
    let src_path = project.root.join("test.rs");
    fs::write(&src_path, "fn main() {}").unwrap();
    let file_hash = Importer::compute_file_hash(&src_path).unwrap();
    idx.upsert_files(&[crate::scanner::FileRecord {
        rel_path: "test.rs".to_string(),
        name: "test".to_string(),
        ext: "rs".to_string(),
        size: 12,
        language: "Rust".to_string(),
        category: "source".to_string(),
        lines: 1,
        hash: file_hash,
        mtime: 0.0,
    }]).unwrap();

    let archive_path = dir.path().join("context.zip");
    let options = ExportOptions::default();
    Exporter::export(&project, &idx, &options, &archive_path).unwrap();

    // Modify the file
    fs::write(&src_path, "fn main() { modified }").unwrap();

    let (_dir2, project2, cfg2) = setup();
    let _idx2 = Index::open(&project2, &cfg2).unwrap();

    let import_opts = ImportOptions {
        allow_overwrite: true,
        skip_stale: true,
        verify_hashes: true,
    };
    let result = Importer::import(&archive_path, &project2, &import_opts).unwrap();

    assert_eq!(result.changed_files.len(), 1);
    assert_eq!(result.changed_files[0].status, "file_missing");
}

#[test]
fn test_export_import_preserves_metadata() {
    let (dir, project, cfg) = setup();
    let idx = Index::open(&project, &cfg).unwrap();

    let mem = crate::memory::MemoryManager::new(&project, &cfg.context).unwrap();
    mem.project_set("meta_key", "meta_value", crate::memory::Provenance::AiGenerated, "metadata", None, vec!["a".into(), "b".into()]).unwrap();

    let archive_path = dir.path().join("context.zip");
    let options = ExportOptions::default();
    Exporter::export(&project, &idx, &options, &archive_path).unwrap();

    let (_dir2, project2, cfg2) = setup();
    let _idx2 = Index::open(&project2, &cfg2).unwrap();

    let import_opts = ImportOptions {
        allow_overwrite: true,
        skip_stale: true,
        verify_hashes: true,
    };
    Importer::import(&archive_path, &project2, &import_opts).unwrap();

    let mem2 = crate::memory::MemoryManager::new(&project2, &cfg2.context).unwrap();
    let entry = mem2.project_get("meta_key").unwrap().unwrap();
    assert_eq!(entry.value, "meta_value");
    assert_eq!(entry.category, "metadata");
    assert_eq!(entry.tags.len(), 2);
    assert_eq!(entry.tags, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn test_import_no_silent_overwrite() {
    let (dir, project, cfg) = setup();
    let idx = Index::open(&project, &cfg).unwrap();

    let mem = crate::memory::MemoryManager::new(&project, &cfg.context).unwrap();
    mem.project_set("exists", "original", crate::memory::Provenance::DeveloperConfirmed, "test", None, vec![]).unwrap();

    let archive_path = dir.path().join("context.zip");
    let options = ExportOptions::default();
    Exporter::export(&project, &idx, &options, &archive_path).unwrap();

    let (_dir2, project2, cfg2) = setup();
    let _idx2 = Index::open(&project2, &cfg2).unwrap();

    let mem2 = crate::memory::MemoryManager::new(&project2, &cfg2.context).unwrap();
    mem2.project_set("exists", "should_not_be_overwritten", crate::memory::Provenance::Detected, "test", None, vec![]).unwrap();

    let import_opts = ImportOptions::default();
    let result = Importer::import(&archive_path, &project2, &import_opts).unwrap();

    assert!(!result.warnings.is_empty());
    let entry = mem2.project_get("exists").unwrap().unwrap();
    assert_eq!(entry.value, "should_not_be_overwritten");
}

#[test]
fn test_import_allow_overwrite() {
    let (dir, project, cfg) = setup();
    let idx = Index::open(&project, &cfg).unwrap();

    let mem = crate::memory::MemoryManager::new(&project, &cfg.context).unwrap();
    mem.project_set("exists", "original", crate::memory::Provenance::DeveloperConfirmed, "test", None, vec![]).unwrap();

    let archive_path = dir.path().join("context.zip");
    let options = ExportOptions::default();
    Exporter::export(&project, &idx, &options, &archive_path).unwrap();

    let (_dir2, project2, cfg2) = setup();
    let _idx2 = Index::open(&project2, &cfg2).unwrap();

    let mem2 = crate::memory::MemoryManager::new(&project2, &cfg2.context).unwrap();
    mem2.project_set("exists", "should_be_overwritten", crate::memory::Provenance::Detected, "test", None, vec![]).unwrap();

    let import_opts = ImportOptions {
        allow_overwrite: true,
        skip_stale: true,
        verify_hashes: true,
    };
    let result = Importer::import(&archive_path, &project2, &import_opts).unwrap();

    assert_eq!(result.project_memory_entries, 1);
    let entry = mem2.project_get("exists").unwrap().unwrap();
    assert_eq!(entry.value, "original");
}

#[test]
fn test_export_skips_project_memory() {
    let (dir, project, cfg) = setup();
    let idx = Index::open(&project, &cfg).unwrap();

    let mem = crate::memory::MemoryManager::new(&project, &cfg.context).unwrap();
    mem.project_set("key1", "value1", crate::memory::Provenance::Detected, "test", None, vec![]).unwrap();

    let archive_path = dir.path().join("context.zip");
    let options = ExportOptions {
        include_project_memory: false,
        include_sessions: true,
    };
    let result = Exporter::export(&project, &idx, &options, &archive_path).unwrap();
    assert_eq!(result.project_memory_entries, 0);
}

#[test]
fn test_export_skips_sessions() {
    let (dir, project, cfg) = setup();
    let idx = Index::open(&project, &cfg).unwrap();

    let mem = crate::memory::MemoryManager::new(&project, &cfg.context).unwrap();
    mem.session_set("sess1", "key1", "value1", crate::memory::Provenance::Detected, None, vec![]).unwrap();

    let archive_path = dir.path().join("context.zip");
    let options = ExportOptions {
        include_project_memory: true,
        include_sessions: false,
    };
    let result = Exporter::export(&project, &idx, &options, &archive_path).unwrap();
    assert_eq!(result.sessions_exported, 0);
}

#[test]
fn test_verify_invalid_archive() {
    let dir = tempdir().unwrap();
    let invalid_path = dir.path().join("invalid.zip");
    std::fs::write(&invalid_path, b"not a zip file").unwrap();

    let result = Importer::verify_archive(&invalid_path);
    assert!(result.is_err());
}

#[test]
fn test_verify_nonexistent_archive() {
    let result = Importer::verify_archive(std::path::Path::new("/nonexistent/archive.zip"));
    assert!(result.is_err());
}

#[test]
fn test_export_import_no_silent_deletion() {
    let (dir, project, cfg) = setup();
    let idx = Index::open(&project, &cfg).unwrap();

    let mem = crate::memory::MemoryManager::new(&project, &cfg.context).unwrap();
    mem.project_set("keep", "data", crate::memory::Provenance::DeveloperConfirmed, "test", None, vec![]).unwrap();
    mem.project_set("export_this", "data", crate::memory::Provenance::DeveloperConfirmed, "test", None, vec![]).unwrap();

    let archive_path = dir.path().join("context.zip");
    let options = ExportOptions::default();
    Exporter::export(&project, &idx, &options, &archive_path).unwrap();

    let existing_data = mem.project_get("keep").unwrap().unwrap();
    assert_eq!(existing_data.value, "data");
}

#[test]
fn test_import_detects_changed_files() -> Result<(), Box<dyn std::error::Error>> {
    let (dir, project, cfg) = setup();
    let idx = Index::open(&project, &cfg).unwrap();

    // Create a source file and add it to the index with a known hash
    let src_path = project.root.join("test.rs");
    fs::write(&src_path, "fn main() {}").unwrap();
    let file_hash = Importer::compute_file_hash(&src_path).unwrap();
    idx.upsert_files(&[crate::scanner::FileRecord {
        rel_path: "test.rs".to_string(),
        name: "test".to_string(),
        ext: "rs".to_string(),
        size: 12,
        language: "Rust".to_string(),
        category: "source".to_string(),
        lines: 1,
        hash: file_hash.clone(),
        mtime: 0.0,
    }])?;

    // Export with the indexed file
    let archive_path = dir.path().join("context.zip");
    let options = ExportOptions::default();
    Exporter::export(&project, &idx, &options, &archive_path).unwrap();

    // Get the manifest and corrupt one hash
    let manifest_data = {
        let file = std::fs::File::open(&archive_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        let mut buf = Vec::new();
        archive.by_name("manifest.json").unwrap().read_to_end(&mut buf).unwrap();
        buf
    };

    let mut manifest: ExportManifest = serde_json::from_slice(&manifest_data).unwrap();
    if let Some(f) = manifest.files.first_mut() {
        f.sha256 = "incorrect_hash_value".to_string();
    }

    // Create a new project with the file (but not indexed) and detect changes
    let (_dir2, project2, cfg2) = setup();
    let idx2 = Index::open(&project2, &cfg2).unwrap();

    let changed = Importer::detect_changes(&project2, &idx2, &manifest.files).unwrap();
    assert!(!changed.is_empty());
    assert_eq!(changed[0].status, "missing");
    Ok(())
}

#[test]
fn test_export_format_version() {
    let fmt = ExportFormat {
        version: EXPORT_FORMAT_VERSION.to_string(),
        created_at: 12345,
        project_root: "/test".to_string(),
        format: "zip".to_string(),
    };
    assert_eq!(fmt.version, "1.0");
}

#[test]
fn test_import_empty_archive_fails() {
    let dir = tempdir().unwrap();
    let empty_path = dir.path().join("empty.zip");

    let file = std::fs::File::create(&empty_path).unwrap();
    let mut zip = ZipWriter::new(std::io::BufWriter::new(file));
    let _ = zip.finish();

    let result = Importer::verify_archive(&empty_path);
    assert!(!result.unwrap_or(false));
}

#[test]
fn test_import_version_mismatch() {
    let (dir, project, cfg) = setup();
    let idx = Index::open(&project, &cfg).unwrap();

    let archive_path = dir.path().join("context.zip");
    let options = ExportOptions::default();
    let result = Exporter::export(&project, &idx, &options, &archive_path).unwrap();
    assert!(!result.archive_path.is_empty());

    let (_dir2, project2, cfg2) = setup();
    let _idx2 = Index::open(&project2, &cfg2).unwrap();

    let import_opts = ImportOptions::default();
    let result = Importer::import(&archive_path, &project2, &import_opts).unwrap();
    assert_eq!(result.project_memory_entries, 0);
}
