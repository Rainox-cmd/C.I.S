use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::cmp::Ordering;

pub const CURRENT_SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone)]
pub struct MigrationVersion {
    pub version: u32,
    pub description: String,
    pub up_sql: String,
    pub down_sql: String,
}

pub struct Migrator;

impl Migrator {
    pub fn migrations() -> Vec<MigrationVersion> {
        vec![
            MigrationVersion {
                version: 1,
                description: "Initial schema".to_string(),
                up_sql: r#"
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    rel_path TEXT UNIQUE NOT NULL,
    name TEXT,
    ext TEXT,
    size INTEGER,
    language TEXT,
    category TEXT,
    lines INTEGER,
    hash TEXT,
    mtime REAL,
    complexity_score INTEGER,
    complexity_level TEXT,
    risk_level TEXT,
    indexed_at REAL
);
CREATE TABLE IF NOT EXISTS symbols (
    id INTEGER PRIMARY KEY,
    file_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    symbol_type TEXT,
    line INTEGER,
    column INTEGER,
    FOREIGN KEY (file_id) REFERENCES files(id)
);
CREATE TABLE IF NOT EXISTS dependencies (
    id INTEGER PRIMARY KEY,
    source_file_id INTEGER NOT NULL,
    target_file_id INTEGER NOT NULL,
    FOREIGN KEY (source_file_id) REFERENCES files(id),
    FOREIGN KEY (target_file_id) REFERENCES files(id)
);
CREATE TABLE IF NOT EXISTS project_memory (
    id INTEGER PRIMARY KEY,
    key TEXT UNIQUE NOT NULL,
    value TEXT NOT NULL,
    provenance TEXT NOT NULL,
    created_at REAL NOT NULL,
    updated_at REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS session_memory (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    provenance TEXT NOT NULL,
    created_at REAL NOT NULL,
    ttl REAL
);
CREATE INDEX IF NOT EXISTS idx_files_rel_path ON files(rel_path);
CREATE INDEX IF NOT EXISTS idx_symbols_file_id ON symbols(file_id);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_dependencies_source ON dependencies(source_file_id);
CREATE INDEX IF NOT EXISTS idx_dependencies_target ON dependencies(target_file_id);
CREATE INDEX IF NOT EXISTS idx_session_memory_session ON session_memory(session_id);
CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(rel_path, name, content);
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    description TEXT,
    applied_at REAL
);
                "#.to_string(),
                down_sql: "DROP TABLE IF EXISTS files; DROP TABLE IF EXISTS symbols; DROP TABLE IF EXISTS dependencies; DROP TABLE IF EXISTS project_memory; DROP TABLE IF EXISTS session_memory; DROP TABLE IF EXISTS files_fts; DROP TABLE IF EXISTS schema_version;".to_string(),
            },
            MigrationVersion {
                version: 2,
                description: "Add edges table for dependency graph".to_string(),
                up_sql: r#"
CREATE TABLE IF NOT EXISTS edges (
    id INTEGER PRIMARY KEY,
    source_file_id INTEGER NOT NULL,
    target_file_id INTEGER NOT NULL,
    source_symbol TEXT,
    target_symbol TEXT,
    dep_type TEXT,
    line INTEGER,
    FOREIGN KEY (source_file_id) REFERENCES files(id),
    FOREIGN KEY (target_file_id) REFERENCES files(id)
);
CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source_file_id);
CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target_file_id);
CREATE INDEX IF NOT EXISTS idx_edges_type ON edges(dep_type);
                "#.to_string(),
                down_sql: "DROP TABLE IF EXISTS edges;".to_string(),
            },
            MigrationVersion {
                version: 3,
                description: "Add git metadata columns to files table".to_string(),
                up_sql: r#"
                ALTER TABLE files ADD COLUMN commit_hash TEXT;
                ALTER TABLE files ADD COLUMN index_stage TEXT;
                "#.to_string(),
                down_sql: r#"
CREATE TABLE files_temp AS SELECT id, rel_path, name, ext, size, language, category, lines, hash, mtime, complexity_score, complexity_level, risk_level, indexed_at FROM files;
DROP TABLE files;
CREATE TABLE files (
    id INTEGER PRIMARY KEY,
    rel_path TEXT UNIQUE NOT NULL,
    name TEXT,
    ext TEXT,
    size INTEGER,
    language TEXT,
    category TEXT,
    lines INTEGER,
    hash TEXT,
    mtime REAL,
    complexity_score INTEGER,
    complexity_level TEXT,
    risk_level TEXT,
    indexed_at REAL
);
INSERT INTO files (id, rel_path, name, ext, size, language, category, lines, hash, mtime, complexity_score, complexity_level, risk_level, indexed_at)
SELECT id, rel_path, name, ext, size, language, category, lines, hash, mtime, complexity_score, complexity_level, risk_level, indexed_at FROM files_temp;
DROP TABLE files_temp;
                "#.to_string(),
            },
        ]
    }

    pub fn current_version(conn: &Connection) -> Result<u32> {
        let table_exists: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if table_exists == 0 {
            return Ok(0);
        }
        let version: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .context("Failed to query schema version")?;
        Ok(version as u32)
    }

    pub fn migrate(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, description TEXT, applied_at REAL);",
        )
        .context("Failed to create schema_version table")?;

        let current = Self::current_version(conn)?;

        for m in &Self::migrations() {
            if m.version > current {
                conn.execute_batch(&m.up_sql)
                    .with_context(|| format!("Failed to apply migration v{}", m.version))?;

                let now = Self::timestamp_now();
                conn.execute(
                    "INSERT OR REPLACE INTO schema_version (version, description, applied_at) VALUES (?1, ?2, ?3)",
                    params![m.version as i64, m.description.as_str(), now],
                )
                    .with_context(|| format!("Failed to record migration v{}", m.version))?;
            }
        }

        Ok(())
    }

    pub fn version_count() -> u32 {
        Self::migrations().len() as u32
    }

    fn timestamp_now() -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }

    pub fn version_compare(a: u32, b: u32) -> Ordering {
        a.cmp(&b)
    }
}

pub fn version_str(version: u32) -> &'static str {
    match version {
        1 => "1.0.0",
        2 => "1.1.0",
        3 => "1.2.0",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests;
