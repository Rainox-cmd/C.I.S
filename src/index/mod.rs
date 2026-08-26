use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};
use std::collections::HashMap;

use crate::{config::Config, project::Project, scanner::FileRecord};

pub type SymbolBatch = Vec<(String, String, u32, u32)>;

pub struct Index {
    conn: Connection,
}

pub struct StoredSymbol {
    pub name: String,
    pub symbol_type: String,
    pub line: i64,
    pub column: i64,
    pub rel_path: String,
}

pub struct SearchResult {
    pub rel_path: String,
    pub name: String,
    pub symbol_type: String,
    pub line: i64,
    pub rank: f64,
}

pub struct Dependency {
    pub source_file: String,
    pub target_file: String,
}

impl Index {
    pub fn open(project: &Project, _config: &Config) -> Result<Self> {
        let conn = Connection::open(project.db_path()).with_context(|| {
            format!("Failed to open database at {}", project.db_path().display())
        })?;

        if _config.database.wal_mode {
            conn.execute_batch("PRAGMA journal_mode=WAL;").context("Failed to set WAL mode")?;
        }

        conn.execute_batch("PRAGMA foreign_keys = ON;").context("Failed to enable foreign keys")?;

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

            tx.execute(
                "INSERT OR REPLACE INTO files_fts (rel_path, name, content)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![
                    &file.rel_path,
                    &file.name,
                    format!("{} {}", file.rel_path, file.name),
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_files(&self, rel_paths: &[String]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for rel_path in rel_paths {
            let file_id: i64 = tx
                .query_row("SELECT id FROM files WHERE rel_path = ?1", rusqlite::params![rel_path], |row| {
                    row.get(0)
                })
                .optional()?
                .unwrap_or(-1);

            if file_id > 0 {
                tx.execute("DELETE FROM symbols WHERE file_id = ?1", rusqlite::params![file_id])?;
                tx.execute(
                    "DELETE FROM dependencies WHERE source_file_id = ?1 OR target_file_id = ?1",
                    rusqlite::params![file_id],
                )?;
                tx.execute("DELETE FROM files_fts WHERE rel_path = ?1", rusqlite::params![rel_path])?;
            }

            tx.execute("DELETE FROM files WHERE rel_path = ?1", rusqlite::params![rel_path])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn upsert_symbols(
        &self,
        rel_path: &str,
        symbols: &[(String, String, u32, u32)],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        let file_id: i64 = tx
            .query_row(
                "SELECT id FROM files WHERE rel_path = ?1",
                rusqlite::params![rel_path],
                |row| row.get(0),
            )
            .context("File not found in index; cannot store symbols")?;

        tx.execute(
            "DELETE FROM symbols WHERE file_id = ?1",
            rusqlite::params![file_id],
        )?;
        tx.execute("DELETE FROM files_fts WHERE rel_path = ?1 AND name != ''", rusqlite::params![rel_path])?;

        for (name, kind, line, column) in symbols.iter() {
            tx.execute(
                "INSERT INTO symbols (file_id, name, symbol_type, line, column)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![file_id, name, kind, line, column],
            )?;

            tx.execute(
                "INSERT INTO files_fts (rel_path, name, content)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![rel_path, name, format!("{}:{}:{}", rel_path, name, kind)],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn upsert_symbols_for_files(
        &self,
        files: &[(&str, &SymbolBatch)],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        for (rel_path, symbols) in files {
            let file_id: i64 = tx
                .query_row(
                    "SELECT id FROM files WHERE rel_path = ?1",
                    rusqlite::params![rel_path],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or(-1);

            if file_id > 0 {
                tx.execute("DELETE FROM symbols WHERE file_id = ?1", rusqlite::params![file_id])?;
                tx.execute("DELETE FROM files_fts WHERE rel_path = ?1 AND name != ''", rusqlite::params![rel_path])?;

                for (name, kind, line, column) in symbols.iter() {
                    tx.execute(
                        "INSERT INTO symbols (file_id, name, symbol_type, line, column)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![file_id, name, kind, line, column],
                    )?;
                    tx.execute(
                        "INSERT INTO files_fts (rel_path, name, content)
                         VALUES (?1, ?2, ?3)",
                        rusqlite::params![rel_path, name, format!("{}:{}:{}", rel_path, name, kind)],
                    )?;
                }
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn get_symbols_by_file(&self, rel_path: &str) -> Result<Vec<StoredSymbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.name, s.symbol_type, s.line, s.column, f.rel_path
             FROM symbols s
             JOIN files f ON s.file_id = f.id
             WHERE f.rel_path = ?1
             ORDER BY s.line",
        )?;
        let rows = stmt.query_map([rel_path], |row| {
            Ok(StoredSymbol {
                name: row.get(0)?,
                symbol_type: row.get(1)?,
                line: row.get(2)?,
                column: row.get(3)?,
                rel_path: row.get(4)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to retrieve symbols")
    }

    pub fn find_symbols_by_name(&self, name: &str) -> Result<Vec<StoredSymbol>> {
        let pattern = format!("%{}%", name);
        let mut stmt = self.conn.prepare(
            "SELECT s.name, s.symbol_type, s.line, s.column, f.rel_path
             FROM symbols s
             JOIN files f ON s.file_id = f.id
             WHERE s.name LIKE ?1
             ORDER BY s.line",
        )?;
        let rows = stmt.query_map([&pattern], |row| {
            Ok(StoredSymbol {
                name: row.get(0)?,
                symbol_type: row.get(1)?,
                line: row.get(2)?,
                column: row.get(3)?,
                rel_path: row.get(4)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to search symbols")
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let escaped = query.replace("'", "''");
        let sql = "SELECT rel_path, name, content, rank
             FROM files_fts
             WHERE files_fts MATCH ?1
             ORDER BY rank";
        let mut stmt = self.conn.prepare(sql)?;

        let rows = stmt.query_map([escaped.as_str()], |row| {
            let rel_path: String = row.get(0)?;
            let name: String = row.get(1)?;
            let _content: String = row.get(2)?;
            let rank: f64 = row.get(3)?;

            let symbol_type = if !name.is_empty() && name != rel_path {
                "symbol".to_string()
            } else {
                "file".to_string()
            };

            Ok(SearchResult {
                rel_path,
                name,
                symbol_type,
                line: 0,
                rank,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            let mut r = row?;
            if r.symbol_type == "symbol" {
                let symbol = self.find_symbol_exact(&r.name, &r.rel_path)?;
                if let Some(s) = symbol {
                    r.line = s.line;
                    r.symbol_type = s.symbol_type;
                }
            }
            results.push(r);
        }

        Ok(results)
    }

    fn find_symbol_exact(&self, name: &str, rel_path: &str) -> Result<Option<StoredSymbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.name, s.symbol_type, s.line, s.column, f.rel_path
             FROM symbols s
             JOIN files f ON s.file_id = f.id
             WHERE s.name = ?1 AND f.rel_path = ?2
             LIMIT 1",
        )?;
        let result = stmt
            .query_row([name, rel_path], |row| {
                Ok(StoredSymbol {
                    name: row.get(0)?,
                    symbol_type: row.get(1)?,
                    line: row.get(2)?,
                    column: row.get(3)?,
                    rel_path: row.get(4)?,
                })
            })
            .optional()?;

        Ok(result)
    }

    pub fn upsert_dependencies(&self, source_rel_path: &str, target_rel_paths: &[String]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        let source_id: i64 = tx
            .query_row(
                "SELECT id FROM files WHERE rel_path = ?1",
                rusqlite::params![source_rel_path],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(-1);

        if source_id > 0 {
            tx.execute(
                "DELETE FROM dependencies WHERE source_file_id = ?1",
                rusqlite::params![source_id],
            )?;

            for target in target_rel_paths {
                let target_id: i64 = tx
                    .query_row(
                        "SELECT id FROM files WHERE rel_path = ?1",
                        rusqlite::params![target],
                        |row| row.get(0),
                    )
                    .optional()?
                    .unwrap_or(-1);

                if target_id > 0 {
                    tx.execute(
                        "INSERT INTO dependencies (source_file_id, target_file_id)
                         VALUES (?1, ?2)",
                        rusqlite::params![source_id, target_id],
                    )?;
                }
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn get_dependencies(&self, rel_path: &str) -> Result<Vec<Dependency>> {
        let mut stmt = self.conn.prepare(
            "SELECT sf.rel_path, tf.rel_path
             FROM dependencies d
             JOIN files sf ON d.source_file_id = sf.id
             JOIN files tf ON d.target_file_id = tf.id
             WHERE sf.rel_path = ?1",
        )?;
        let rows = stmt.query_map([rel_path], |row| {
            Ok(Dependency {
                source_file: row.get(0)?,
                target_file: row.get(1)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to retrieve dependencies")
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

            DROP TABLE IF EXISTS files_fts;
            CREATE VIRTUAL TABLE files_fts USING fts5(
                rel_path, name, content
            );
            ",
            )
            .context("Failed to create database schema")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests;
