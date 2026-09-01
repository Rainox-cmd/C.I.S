use super::*;
use crate::config::{Config, ContextConfig};
use crate::project::Project;
use tempfile::tempdir;

fn setup() -> (tempfile::TempDir, Project, ContextConfig, MemoryManager) {
    let dir = tempdir().unwrap();
    let project = Project::new(dir.path().to_path_buf()).unwrap();
    project.init().unwrap();
    let config = Config::default();
    let memory = MemoryManager::new(&project, &config.context).unwrap();
    (dir, project, config.context, memory)
}

#[test]
fn test_memory_project_crud() {
    let (_dir, _project, _cfg, memory) = setup();

    memory.project_set("archetype", "microservice", Provenance::Detected, "architecture", None, vec!["backend".into()]).unwrap();
    let entry = memory.project_get("archetype").unwrap().unwrap();
    assert_eq!(entry.value, "microservice");
    assert_eq!(entry.provenance, Provenance::Detected);
    assert_eq!(entry.category, "architecture");
    assert_eq!(entry.tags, vec!["backend"]);

    memory.project_set("archetype", "monolith", Provenance::DeveloperConfirmed, "architecture", None, vec![]).unwrap();
    let entry = memory.project_get("archetype").unwrap().unwrap();
    assert_eq!(entry.value, "monolith");
    assert_eq!(entry.provenance, Provenance::DeveloperConfirmed);

    let deleted = memory.project_delete("archetype").unwrap();
    assert!(deleted);
    assert!(memory.project_get("archetype").unwrap().is_none());
    let deleted_again = memory.project_delete("archetype").unwrap();
    assert!(!deleted_again);
}

#[test]
fn test_memory_provenance_tracking() {
    let (_dir, _project, _cfg, memory) = setup();

    memory.project_set("p1", "v1", Provenance::Detected, "test", None, vec![]).unwrap();
    memory.project_set("p2", "v2", Provenance::Inferred, "test", None, vec![]).unwrap();
    memory.project_set("p3", "v3", Provenance::AiGenerated, "test", None, vec![]).unwrap();
    memory.project_set("p4", "v4", Provenance::DeveloperConfirmed, "test", None, vec![]).unwrap();

    assert_eq!(memory.project_get("p1").unwrap().unwrap().provenance, Provenance::Detected);
    assert_eq!(memory.project_get("p2").unwrap().unwrap().provenance, Provenance::Inferred);
    assert_eq!(memory.project_get("p3").unwrap().unwrap().provenance, Provenance::AiGenerated);
    assert_eq!(memory.project_get("p4").unwrap().unwrap().provenance, Provenance::DeveloperConfirmed);
}

#[test]
fn test_memory_ttl_expiration() {
    let (_dir, _project, _cfg, memory) = setup();

    memory.project_set("temp", "expires_soon", Provenance::Detected, "test", Some(1), vec![]).unwrap();

    assert!(memory.project_get("temp").unwrap().is_some());

    std::thread::sleep(std::time::Duration::from_secs(2));

    assert!(memory.project_get("temp").unwrap().is_none());
}

#[test]
fn test_memory_list() {
    let (_dir, _project, _cfg, memory) = setup();

    memory.project_set("b_key", "v1", Provenance::Detected, "cat1", None, vec![]).unwrap();
    memory.project_set("a_key", "v2", Provenance::Detected, "cat1", None, vec![]).unwrap();
    memory.project_set("c_key", "v3", Provenance::Detected, "cat2", None, vec![]).unwrap();

    let entries = memory.project_list().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].key, "a_key");
    assert_eq!(entries[1].key, "b_key");
    assert_eq!(entries[2].key, "c_key");
}

#[test]
fn test_memory_project_isolation() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();

    let proj1 = Project::new(dir1.path().to_path_buf()).unwrap();
    proj1.init().unwrap();
    let proj2 = Project::new(dir2.path().to_path_buf()).unwrap();
    proj2.init().unwrap();

    let cfg = Config::default();
    let mem1 = MemoryManager::new(&proj1, &cfg.context).unwrap();
    let mem2 = MemoryManager::new(&proj2, &cfg.context).unwrap();

    mem1.project_set("shared_key", "project1_value", Provenance::Detected, "test", None, vec![]).unwrap();
    mem2.project_set("shared_key", "project2_value", Provenance::Detected, "test", None, vec![]).unwrap();

    assert_eq!(mem1.project_get("shared_key").unwrap().unwrap().value, "project1_value");
    assert_eq!(mem2.project_get("shared_key").unwrap().unwrap().value, "project2_value");
}

#[test]
fn test_memory_session_crud() {
    let (_dir, _project, _cfg, memory) = setup();

    memory.session_set("sess1", "task", "Analyze code", Provenance::Detected, None, vec![]).unwrap();
    memory.session_set("sess1", "context", "Rust codebase", Provenance::Inferred, None, vec!["code".into()]).unwrap();

    let entry = memory.session_get("sess1", "task").unwrap().unwrap();
    assert_eq!(entry.value, "Analyze code");

    let entries = memory.session_list("sess1").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "context");
    assert_eq!(entries[1].key, "task");

    let deleted = memory.session_delete("sess1", "context").unwrap();
    assert!(deleted);
    assert_eq!(memory.session_list("sess1").unwrap().len(), 1);
}

#[test]
fn test_memory_budget_enforcement() {
    let (_dir, _project, _cfg, memory) = setup();

    let result = memory.project_set(
        "oversized",
        &"x".repeat((memory.config.max_session_context_bytes + 1) as usize),
        Provenance::Detected,
        "test",
        None,
        vec![],
    );
    assert!(result.is_err());
}

#[test]
fn test_memory_storage_limits_config() {
    let cfg = ContextConfig {
        max_session_context_bytes: 10 * 1024 * 1024,
        max_sessions: 50,
        max_total_context_bytes: 500 * 1024 * 1024,
        max_context_file_bytes: 1024 * 1024,
        session_ttl_seconds: 7 * 24 * 60 * 60,
    };
    assert_eq!(cfg.max_session_context_bytes, 10_485_760);
    assert_eq!(cfg.max_sessions, 50);
    assert_eq!(cfg.max_total_context_bytes, 524_288_000);
    assert_eq!(cfg.max_context_file_bytes, 1_048_576);
    assert_eq!(cfg.session_ttl_seconds, 604_800);
}

#[test]
fn test_memory_export_before_delete() {
    let dir = tempdir().unwrap();
    let project = Project::new(dir.path().to_path_buf()).unwrap();
    project.init().unwrap();
    let mut config = Config::default();
    config.context.session_ttl_seconds = 1;
    let memory = MemoryManager::new(&project, &config.context).unwrap();

    memory.session_set("sess1", "key1", "value1", Provenance::Detected, None, vec![]).unwrap();
    memory.session_set("sess1", "key2", "value2", Provenance::Detected, None, vec![]).unwrap();

    std::thread::sleep(std::time::Duration::from_secs(2));

    let expired = memory.expired_sessions().unwrap();
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0], "sess1");

    let pruned = memory.prune_expired_sessions().unwrap();
    assert_eq!(pruned.len(), 1);
    assert_eq!(pruned[0], "sess1");

    let sessions = memory.list_sessions().unwrap();
    assert!(sessions.is_empty());

    let archive_path = project.cache_dir.join("deleted_sessions").join("session-sess1.tar");
    assert!(archive_path.exists());
    let archive_content = fs::read_to_string(&archive_path).unwrap();
    let archived_entries: Vec<MemoryEntry> = serde_json::from_str(&archive_content).unwrap();
    assert_eq!(archived_entries.len(), 2);
}

#[test]
fn test_memory_no_silent_deletion() {
    let (_dir, _project, _cfg, memory) = setup();

    memory.project_set("keep_me", "important", Provenance::DeveloperConfirmed, "decisions", None, vec![]).unwrap();
    memory.session_set("sess1", "task", "working", Provenance::Detected, None, vec![]).unwrap();

    let before_sessions = memory.list_sessions().unwrap();
    assert_eq!(before_sessions.len(), 1);

    let _ = memory.project_delete("keep_me").unwrap();

    assert!(memory.project_get("keep_me").unwrap().is_none());
    assert_eq!(memory.list_sessions().unwrap().len(), 1);
}

#[test]
fn test_memory_warning_system() {
    let dir = tempdir().unwrap();
    let project = Project::new(dir.path().to_path_buf()).unwrap();
    project.init().unwrap();

    let config = ContextConfig {
        max_session_context_bytes: 1024 * 1024,
        max_sessions: 2,
        max_total_context_bytes: 10 * 1024 * 1024,
        max_context_file_bytes: 100,
        session_ttl_seconds: 60,
    };

    let memory = MemoryManager::new(&project, &config).unwrap();

    let warnings = memory.warn_if_near_limits();
    assert!(warnings.is_empty());

    let small_config = ContextConfig {
        max_session_context_bytes: 10,
        max_sessions: 5,
        max_total_context_bytes: 100,
        max_context_file_bytes: 100,
        session_ttl_seconds: 60,
    };
    let small_memory = MemoryManager::new(&project, &small_config).unwrap();
    let warnings = small_memory.warn_if_near_limits();
    assert!(warnings.is_empty());
}

#[test]
fn test_memory_storage_usage() {
    let (_dir, _project, _cfg, memory) = setup();

    memory.project_set("key1", "value1", Provenance::Detected, "test", None, vec![]).unwrap();

    let (project_size, _total_size, session_count) = memory.get_storage_usage().unwrap();
    assert!(project_size > 0);
    assert_eq!(session_count, 0);
}

#[test]
fn test_memory_check_limits() {
    let (_dir, _project, _cfg, memory) = setup();

    memory.project_set("key1", "value1", Provenance::Detected, "test", None, vec![]).unwrap();
    assert!(memory.check_limits().is_ok());
}

#[test]
fn test_memory_session_ttl() {
    let dir = tempdir().unwrap();
    let project = Project::new(dir.path().to_path_buf()).unwrap();
    project.init().unwrap();
    let mut config = Config::default();
    config.context.session_ttl_seconds = 1;
    let memory = MemoryManager::new(&project, &config.context).unwrap();

    memory.session_set("sess1", "task", "work", Provenance::Detected, None, vec![]).unwrap();

    assert!(memory.session_get("sess1", "task").unwrap().is_some());

    std::thread::sleep(std::time::Duration::from_secs(2));

    let expired = memory.expired_sessions().unwrap();
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0], "sess1");

    memory.prune_expired_sessions().unwrap();

    assert!(memory.session_get("sess1", "task").unwrap().is_none());
}

#[test]
fn test_memory_session_does_not_expire_on_single_entry() {
    let (_dir, _project, _cfg, memory) = setup();

    memory.session_set("sess1", "temp", "short-lived", Provenance::Detected, Some(1), vec![]).unwrap();
    memory.session_set("sess1", "perm", "long-lived", Provenance::Detected, None, vec![]).unwrap();

    std::thread::sleep(std::time::Duration::from_secs(2));

    // The temporary entry should expire on access
    assert!(memory.session_get("sess1", "temp").unwrap().is_none());
    
    // The permanent entry should remain
    assert!(memory.session_get("sess1", "perm").unwrap().is_some());

    // The session should NOT be in expired sessions
    let expired = memory.expired_sessions().unwrap();
    assert!(expired.is_empty(), "Session expired incorrectly due to a single entry");

    // Pruning should not delete the session
    let pruned = memory.prune_expired_sessions().unwrap();
    assert!(pruned.is_empty());
}

#[test]
fn test_memory_recent_activity_keeps_session_alive() {
    let dir = tempdir().unwrap();
    let project = Project::new(dir.path().to_path_buf()).unwrap();
    project.init().unwrap();
    let mut config = Config::default();
    config.context.session_ttl_seconds = 2; // 2 seconds ttl
    let memory = MemoryManager::new(&project, &config.context).unwrap();

    memory.session_set("sess3", "old", "data", Provenance::Detected, None, vec![]).unwrap();

    // Wait 1 second (not enough to expire the session)
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Add new activity
    memory.session_set("sess3", "new", "data", Provenance::Detected, None, vec![]).unwrap();

    // Wait 1.5 seconds. 
    // By the "new" entry, it is only 1.5s old, which < 2s TTL, so it should NOT expire.
    std::thread::sleep(std::time::Duration::from_millis(1500));

    let expired = memory.expired_sessions().unwrap();
    assert!(expired.is_empty(), "Session expired despite recent activity");
}

#[test]
fn test_memory_config_set_context_limits() {
    let mut cfg = Config::default();
    cfg.set("context.max_sessions", "100").unwrap();
    assert_eq!(cfg.context.max_sessions, 100);
    cfg.set("context.session_ttl_seconds", "3600").unwrap();
    assert_eq!(cfg.context.session_ttl_seconds, 3600);
}

#[test]
fn test_memory_tags() {
    let (_dir, _project, _cfg, memory) = setup();

    memory.project_set("entry1", "value", Provenance::Detected, "test", None, vec!["tag1".into(), "tag2".into()]).unwrap();
    let entry = memory.project_get("entry1").unwrap().unwrap();
    assert_eq!(entry.tags.len(), 2);
    assert_eq!(entry.tags[0], "tag1");
    assert_eq!(entry.tags[1], "tag2");
}
