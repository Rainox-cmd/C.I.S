use super::*;
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn test_migrations_count() {
    let migrations = Migrator::migrations();
    assert_eq!(migrations.len(), 3);
    assert_eq!(CURRENT_SCHEMA_VERSION, 3);
}

#[test]
fn test_migrate_fresh_database() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let conn = Connection::open(&db_path).unwrap();

    Migrator::migrate(&conn).unwrap();

    let version = Migrator::current_version(&conn).unwrap();
    assert_eq!(version, 3);
}

#[test]
fn test_migrate_applies_all_migrations() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let conn = Connection::open(&db_path).unwrap();

    Migrator::migrate(&conn).unwrap();

    // Verify tables exist
    let table_count: i64 = conn
        .query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('files', 'symbols', 'dependencies', 'edges', 'schema_version')", [], |row| row.get(0))
        .unwrap();
    assert_eq!(table_count, 5);
}

#[test]
fn test_migrate_idempotent() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let conn = Connection::open(&db_path).unwrap();

    Migrator::migrate(&conn).unwrap();
    let v1 = Migrator::current_version(&conn).unwrap();

    Migrator::migrate(&conn).unwrap();
    let v2 = Migrator::current_version(&conn).unwrap();

    assert_eq!(v1, v2);
}

#[test]
fn test_version_compare() {
    assert_eq!(Migrator::version_compare(1, 2), std::cmp::Ordering::Less);
    assert_eq!(Migrator::version_compare(2, 1), std::cmp::Ordering::Greater);
    assert_eq!(Migrator::version_compare(3, 3), std::cmp::Ordering::Equal);
}

#[test]
fn test_version_str() {
    assert_eq!(version_str(1), "1.0.0");
    assert_eq!(version_str(2), "1.1.0");
    assert_eq!(version_str(3), "1.2.0");
    assert_eq!(version_str(99), "unknown");
}

#[test]
fn test_migration_descriptions() {
    let migrations = Migrator::migrations();
    assert_eq!(migrations[0].description, "Initial schema");
    assert_eq!(migrations[1].description, "Add edges table for dependency graph");
    assert_eq!(migrations[2].description, "Add git metadata columns to files table");
}

#[test]
fn test_migration_version_sequence() {
    let migrations = Migrator::migrations();
    for i in 0..migrations.len() - 1 {
        assert_eq!(
            migrations[i].version,
            migrations[i + 1].version - 1,
            "Migration versions should be sequential"
        );
    }
}

#[test]
fn test_current_version_empty_db() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("empty.db");
    let conn = Connection::open(&db_path).unwrap();

    let version = Migrator::current_version(&conn).unwrap();
    assert_eq!(version, 0);
}

#[test]
fn test_migration_version_count() {
    assert_eq!(Migrator::version_count(), 3);
}
