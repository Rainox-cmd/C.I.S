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

pub struct Edge {
    pub source_file: String,
    pub target_file: String,
    pub source_symbol: Option<String>,
    pub target_symbol: Option<String>,
    pub dep_type: String,
    pub line: i64,
}

pub struct EntryPoint {
    pub rel_path: String,
    pub symbol_count: i64,
    pub incoming_count: i64,
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
                "INSERT INTO files (rel_path, name, ext, size, language, category, lines, hash, mtime, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(rel_path) DO UPDATE SET
                     name = excluded.name, ext = excluded.ext, size = excluded.size,
                     language = excluded.language, category = excluded.category,
                     lines = excluded.lines, hash = excluded.hash,
                     mtime = excluded.mtime, indexed_at = excluded.indexed_at",
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
                "DELETE FROM files_fts WHERE rel_path = ?1",
                rusqlite::params![&file.rel_path],
            )?;
            tx.execute(
                "INSERT INTO files_fts (rel_path, name, content)
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
                tx.execute(
                    "DELETE FROM edges WHERE source_file_id = ?1 OR target_file_id = ?1",
                    rusqlite::params![file_id],
                )?;
                tx.execute("DELETE FROM files_fts WHERE rel_path = ?1", rusqlite::params![rel_path])?;
            }

            tx.execute("DELETE FROM files WHERE rel_path = ?1", rusqlite::params![rel_path])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn update_git_metadata(&self, rel_path: &str, commit_hash: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE files SET commit_hash = ?1, index_stage = ?2 WHERE rel_path = ?3",
            rusqlite::params![commit_hash, "indexed", rel_path],
        )?;
        Ok(())
    }

    pub fn get_file_git_metadata(&self) -> Result<Vec<(String, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT rel_path, commit_hash FROM files WHERE commit_hash IS NOT NULL"
        )?;
        let rows = stmt.query_map([], |row| {
            let path: String = row.get(0)?;
            let hash: Option<String> = row.get(1)?;
            Ok((path, hash))
        })?;
        rows.collect::<Result<Vec<_>, _>>().context("Failed to retrieve git metadata")
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

        let file_name: String = tx
            .query_row(
                "SELECT name FROM files WHERE rel_path = ?1",
                rusqlite::params![rel_path],
                |row| row.get(0),
            )?;

        tx.execute(
            "DELETE FROM symbols WHERE file_id = ?1",
            rusqlite::params![file_id],
        )?;
        tx.execute("DELETE FROM files_fts WHERE rel_path = ?1", rusqlite::params![rel_path])?;

        tx.execute(
            "INSERT INTO files_fts (rel_path, name, content)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![rel_path, &file_name, format!("{} {}", rel_path, file_name)],
        )?;

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
                let file_name: String = tx
                    .query_row(
                        "SELECT name FROM files WHERE rel_path = ?1",
                        rusqlite::params![rel_path],
                        |row| row.get(0),
                    )
                    .optional()?
                    .unwrap_or_default();

                tx.execute("DELETE FROM symbols WHERE file_id = ?1", rusqlite::params![file_id])?;
                tx.execute("DELETE FROM files_fts WHERE rel_path = ?1", rusqlite::params![rel_path])?;

                if !file_name.is_empty() {
                    tx.execute(
                        "INSERT INTO files_fts (rel_path, name, content)
                         VALUES (?1, ?2, ?3)",
                        rusqlite::params![rel_path, &file_name, format!("{} {}", rel_path, file_name)],
                    )?;
                }

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

    pub fn upsert_symbol_edges(&self, source_rel_path: &str, edges: &[(String, String, String, u32)]) -> Result<()> {
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
                "DELETE FROM edges WHERE source_file_id = ?1",
                rusqlite::params![source_id],
            )?;

            for (target_symbol, target_file, dep_type, line) in edges.iter() {
                let target_id: i64 = tx
                    .query_row(
                        "SELECT id FROM files WHERE rel_path = ?1",
                        rusqlite::params![target_file],
                        |row| row.get(0),
                    )
                    .optional()?
                    .unwrap_or(-1);

                if target_id > 0 {
                    tx.execute(
                        "INSERT INTO edges (source_file_id, target_file_id, source_symbol, target_symbol, dep_type, line)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        rusqlite::params![source_id, target_id, target_symbol, target_symbol, dep_type, *line as i64],
                    )?;
                }
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn get_forward_edges(&self, rel_path: &str) -> Result<Vec<Edge>> {
        let mut stmt = self.conn.prepare(
            "SELECT sf.rel_path, tf.rel_path, e.source_symbol, e.target_symbol, e.dep_type, e.line
             FROM edges e
             JOIN files sf ON e.source_file_id = sf.id
             JOIN files tf ON e.target_file_id = tf.id
             WHERE sf.rel_path = ?1",
        )?;
        let rows = stmt.query_map([rel_path], |row| {
            Ok(Edge {
                source_file: row.get(0)?,
                target_file: row.get(1)?,
                source_symbol: row.get(2)?,
                target_symbol: row.get(3)?,
                dep_type: row.get(4)?,
                line: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().context("Failed to retrieve forward edges")
    }

    pub fn get_reverse_edges(&self, rel_path: &str) -> Result<Vec<Edge>> {
        let mut stmt = self.conn.prepare(
            "SELECT sf.rel_path, tf.rel_path, e.source_symbol, e.target_symbol, e.dep_type, e.line
             FROM edges e
             JOIN files sf ON e.source_file_id = sf.id
             JOIN files tf ON e.target_file_id = tf.id
             WHERE tf.rel_path = ?1",
        )?;
        let rows = stmt.query_map([rel_path], |row| {
            Ok(Edge {
                source_file: row.get(0)?,
                target_file: row.get(1)?,
                source_symbol: row.get(2)?,
                target_symbol: row.get(3)?,
                dep_type: row.get(4)?,
                line: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().context("Failed to retrieve reverse edges")
    }

    pub fn get_transitive_dependencies(&self, rel_path: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "WITH RECURSIVE transitive_deps(target_file_id) AS (
                 SELECT d.target_file_id FROM dependencies d
                 JOIN files sf ON d.source_file_id = sf.id
                 WHERE sf.rel_path = ?1
                 UNION
                 SELECT e.target_file_id FROM edges e
                 JOIN transitive_deps d ON e.source_file_id = d.target_file_id
             )
             SELECT DISTINCT f.rel_path FROM transitive_deps d
             JOIN files f ON d.target_file_id = f.id
             WHERE f.rel_path != ?1",
        )?;
        let rows = stmt.query_map([rel_path], |row| {
            let path: String = row.get(0)?;
            Ok(path)
        })?;
        rows.collect::<Result<Vec<_>, _>>().context("Failed to retrieve transitive dependencies")
    }

    pub fn get_reverse_dependencies(&self, rel_path: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "WITH RECURSIVE revdeps(source_file_id) AS (
                 SELECT e.source_file_id FROM edges e
                 JOIN files tf ON e.target_file_id = tf.id
                 WHERE tf.rel_path = ?1
                 UNION
                 SELECT e.source_file_id FROM edges e
                 JOIN revdeps r ON e.target_file_id = r.source_file_id
             )
             SELECT DISTINCT f.rel_path FROM revdeps r
             JOIN files f ON r.source_file_id = f.id
             WHERE f.rel_path != ?1",
        )?;
        let rows = stmt.query_map([rel_path], |row| {
            let path: String = row.get(0)?;
            Ok(path)
        })?;
        rows.collect::<Result<Vec<_>, _>>().context("Failed to retrieve reverse dependencies")
    }

    pub fn get_entry_points(&self) -> Result<Vec<EntryPoint>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.rel_path,
             (SELECT COUNT(*) FROM symbols WHERE file_id = f.id) as symbol_count,
             (SELECT COUNT(*) FROM edges e JOIN files tf ON e.target_file_id = tf.id WHERE tf.rel_path = f.rel_path) as incoming
             FROM files f
             WHERE f.category = 'source'
             ORDER BY f.rel_path",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(EntryPoint {
                rel_path: row.get(0)?,
                symbol_count: row.get(1)?,
                incoming_count: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().context("Failed to retrieve entry points")
    }

    pub fn has_cycle(&self) -> Result<bool> {
        let mut stmt = self.conn.prepare(
            "WITH RECURSIVE reach(start, current, depth) AS (
                 SELECT e.source_file_id, e.target_file_id, 1 FROM edges e
                 UNION ALL
                 SELECT r.start, e.target_file_id, r.depth + 1
                 FROM reach r
                 JOIN edges e ON r.current = e.source_file_id
                 WHERE r.depth < 100
             )
             SELECT COUNT(*) FROM reach WHERE start = current AND depth > 1",
        )?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count > 0)
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
                commit_hash TEXT,
                index_stage TEXT,
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

            CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
                rel_path, name, content
            );
            ",
        )
        .context("Failed to create database schema")?;

        let needs_rebuild: bool = self.conn.query_row(
            "SELECT count(*) FROM pragma_table_info('files_fts') WHERE name = 'content'",
            [],
            |row| row.get(0),
        ).unwrap_or(0) == 0;
        if needs_rebuild {
            self.conn.execute_batch("DROP TABLE IF EXISTS files_fts;")?;
            self.conn.execute_batch(
                "CREATE VIRTUAL TABLE files_fts USING fts5(rel_path, name, content);",
            )?;
        }

        let has_column: bool = self.conn.query_row(
            "SELECT count(*) FROM pragma_table_info('symbols') WHERE name = 'column'",
            [],
            |row| row.get(0),
        ).unwrap_or(0) == 1;
        if !has_column {
            self.conn.execute_batch("ALTER TABLE symbols ADD COLUMN column INTEGER;")?;
        }

        let has_complexity: bool = self.conn.query_row(
            "SELECT count(*) FROM pragma_table_info('files') WHERE name = 'complexity_score'",
            [],
            |row| row.get(0),
        ).unwrap_or(0) == 1;
        if !has_complexity {
            self.conn.execute_batch(
                "ALTER TABLE files ADD COLUMN complexity_score INTEGER;
                 ALTER TABLE files ADD COLUMN complexity_level TEXT;
                 ALTER TABLE files ADD COLUMN risk_level TEXT;",
            )?;
        }

        let has_edges: bool = self.conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='edges'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0) == 1;
        if !has_edges {
            self.conn.execute_batch(
                "CREATE TABLE edges (
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
                 CREATE INDEX IF NOT EXISTS idx_edges_type ON edges(dep_type);",
            )?;
        }
        let has_commit_hash: bool = self.conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('files') WHERE name = 'commit_hash'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0) == 1;
        if !has_commit_hash {
            self.conn.execute_batch(
                "ALTER TABLE files ADD COLUMN commit_hash TEXT;
                 ALTER TABLE files ADD COLUMN index_stage TEXT;",
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod graph_tests;

#[cfg(test)]
mod incr_tests;
