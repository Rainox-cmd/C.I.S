use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::HashMap;

use crate::{config::Config, project::Project, scanner::FileRecord};

pub struct Index {
    conn: Connection,
}

impl Index {
    pub fn open(project: &Project, _config: &Config) -> Result<Self> {
        let conn = Connection::open(project.db_path()).with_context(|| {
            format!("Failed to open database at {}", project.db_path().display())
        })?;

        if _config.database.wal_mode {
            conn.execute_batch("PRAGMA journal_mode=WAL;")
                .context("Failed to set WAL mode")?;
        }

        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .context("Failed to enable foreign keys")?;

        let index = Self { conn };
        index.ensure_schema()?;
        Ok(index)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn integrity_check(&self) -> Result<bool> {
        let result: String = self
            .conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .context("Failed to run integrity check")?;
        Ok(result == "ok")
    }

    pub fn count_records(&self, table: &str) -> Result<i64> {
        let query = format!("SELECT COUNT(*) FROM {}", table);
        self.conn
            .query_row(&query, [], |row| row.get(0))
            .with_context(|| format!("Failed to count records in {}", table))
    }

    pub fn doctor(&self) -> Result<()> {
        println!("C.I.S. Doctor");
        println!("Database: OK");
        let file_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
            .unwrap_or(0);
        println!("Indexed files: {}", file_count);
        let symbol_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))
            .unwrap_or(0);
        println!("Indexed symbols: {}", symbol_count);
        let dep_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM dependencies", [], |row| row.get(0))
            .unwrap_or(0);
        println!("Indexed dependencies: {}", dep_count);
        Ok(())
    }

    pub fn status(&self) -> Result<()> {
        let file_count = self.count_records("files")?;
        let symbol_count = self.count_records("symbols")?;
        let dep_count = self.count_records("dependencies")?;
        let pm_count = self.count_records("project_memory")?;
        let sm_count = self.count_records("session_memory")?;

        println!("Indexed files: {}", file_count);
        println!("Indexed symbols: {}", symbol_count);
        println!("Indexed dependencies: {}", dep_count);
        println!("Project memory entries: {}", pm_count);
        println!("Session memory entries: {}", sm_count);
        Ok(())
    }

    pub fn get_file_hashes(&self) -> Result<HashMap<String, String>> {
        let mut stmt = self.conn.prepare("SELECT rel_path, hash FROM files")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut hashes = HashMap::new();
        for row in rows {
            let (path, hash) = row?;
            hashes.insert(path, hash);
        }
        Ok(hashes)
    }

    pub fn upsert_files(&self, files: &[FileRecord]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for file in files {
            tx.execute(
                "INSERT OR REPLACE INTO files (rel_path, name, ext, size, language, category, lines, hash, mtime, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    file.rel_path,
                    file.name,
                    file.ext,
                    file.size,
                    file.language,
                    file.category,
                    file.lines,
                    file.hash,
                    file.mtime,
                    chrono::Utc::now().timestamp() as f64,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_files(&self, rel_paths: &[String]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for rel_path in rel_paths {
            tx.execute(
                "DELETE FROM files WHERE rel_path = ?1",
                rusqlite::params![rel_path],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn ensure_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "
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

            CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
                rel_path, name, content=symbols
            );
            ",
            )
            .context("Failed to create database schema")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_index_open_creates_schema() {
        let dir = tempdir().unwrap();
        let project = Project::new(dir.path().to_path_buf()).unwrap();
        project.init().unwrap();
        let cfg = Config::default();
        let idx = Index::open(&project, &cfg).unwrap();

        let count: i64 = idx
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_index_doctor() {
        let dir = tempdir().unwrap();
        let project = Project::new(dir.path().to_path_buf()).unwrap();
        project.init().unwrap();
        let cfg = Config::default();
        let idx = Index::open(&project, &cfg).unwrap();

        let result = idx.doctor();
        assert!(result.is_ok());
    }

    #[test]
    fn test_index_status() {
        let dir = tempdir().unwrap();
        let project = Project::new(dir.path().to_path_buf()).unwrap();
        project.init().unwrap();
        let cfg = Config::default();
        let idx = Index::open(&project, &cfg).unwrap();

        let result = idx.status();
        assert!(result.is_ok());
    }

    #[test]
    fn test_index_upsert_and_get_hashes() {
        let dir = tempdir().unwrap();
        let project = Project::new(dir.path().to_path_buf()).unwrap();
        project.init().unwrap();
        let cfg = Config::default();
        let idx = Index::open(&project, &cfg).unwrap();

        let files = vec![FileRecord {
            rel_path: "src/main.rs".to_string(),
            name: "main.rs".to_string(),
            ext: ".rs".to_string(),
            size: 100,
            language: "Rust".to_string(),
            category: "source".to_string(),
            lines: 10,
            hash: "abc123".to_string(),
            mtime: 1000.0,
        }];

        idx.upsert_files(&files).unwrap();

        let hashes = idx.get_file_hashes().unwrap();
        assert_eq!(hashes.get("src/main.rs"), Some(&"abc123".to_string()));
    }
}
