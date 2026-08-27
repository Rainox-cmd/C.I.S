# C.I.S. API Reference

## Core Types

### `Project`
```rust
pub struct Project {
    pub root: PathBuf,
    pub cis_dir: PathBuf,
    pub db_path: PathBuf,
    pub logs_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub context_dir: PathBuf,
    pub project_memory_dir: PathBuf,
    pub sessions_dir: PathBuf,
}
```

#### Methods
- `Project::discover() -> Result<Self>` — Find project from current directory
- `Project::new(root: PathBuf) -> Result<Self>` — Create project at path
- `Project::init(&self) -> Result<()>` — Initialize .cis directory structure

### `Config`
```rust
pub struct Config {
    pub general: GeneralConfig,
    pub scanner: ScannerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
    pub context: ContextConfig,
}
```

#### Methods
- `Config::default() -> Self` — Default configuration
- `Config::load(project: &Project) -> Result<Self>` — Load from config.toml
- `Config::save(&self, project: &Project) -> Result<()>` — Save to config.toml
- `Config::set(&mut self, key: &str, value: &str) -> Result<()>` — Set config value

### `Index`
```rust
pub struct Index { conn: Connection }
```

#### Methods
- `Index::open(project: &Project, config: &Config) -> Result<Self>` — Open database
- `Index::search(&self, query: &str) -> Result<Vec<SearchResult>>` — FTS5 search
- `Index::find_symbols_by_name(&self, name: &str) -> Result<Vec<StoredSymbol>>` — Find symbols
- `Index::get_dependencies(&self, path: &str) -> Result<Vec<Dependency>>` — Get file dependencies
- `Index::get_reverse_dependencies(&self, path: &str) -> Result<Vec<String>>` — Get impact analysis
- `Index::get_entry_points(&self) -> Result<Vec<EntryPoint>>` — Get entry points
- `Index::has_cycle(&self) -> Result<bool>` — Check for circular dependencies
- `Index::get_file_hashes(&self) -> Result<HashMap<String, String>>` — Get file hashes

### `GitClient`
```rust
pub struct GitClient { /* ... */ }
```

#### Methods
- `GitClient::new(root: &Path) -> Result<Self>`
- `is_repo(&self) -> bool`
- `current_branch(&self) -> Result<String>`
- `status(&self) -> Result<Vec<StatusEntry>>`
- `is_dirty(&self) -> Result<bool>`
- `diff(&self) -> Result<String>` — Unstaged changes
- `diff_staged(&self) -> Result<String>` — Staged changes
- `diff_stat(&self) -> Result<Vec<DiffStat>>` — Diff statistics
- `log(&self, count: usize) -> Result<Vec<CommitInfo>>`
- `file_history(&self, path: &str, count: usize) -> Result<Vec<CommitInfo>>`
- `file_last_commit(&self, path: &str) -> Result<Option<CommitInfo>>`
- `link_file_commit(&self, rel_path: &str, commit_hash: Option<&str>) -> String`

### `MemoryManager`
```rust
pub struct MemoryManager { /* ... */ }
```

#### Methods
- `MemoryManager::new(project: &Project, config: &ContextConfig) -> Result<Self>`
- `project_set(&self, key: &str, value: &str, provenance: Provenance, category: &str, ttl_seconds: Option<u64>, tags: Vec<String>) -> Result<()>`
- `project_get(&self, key: &str) -> Result<Option<MemoryEntry>>`
- `project_list(&self) -> Result<Vec<MemoryEntry>>`
- `project_delete(&self, key: &str) -> Result<bool>`
- `session_set(&self, session_id: &str, key: &str, value: &str, provenance: Provenance, ttl_seconds: Option<u64>, tags: Vec<String>) -> Result<()>`
- `session_get(&self, session_id: &str, key: &str) -> Result<Option<MemoryEntry>>`
- `session_list(&self, session_id: &str) -> Result<Vec<MemoryEntry>>`
- `session_delete(&self, session_id: &str, key: &str) -> Result<bool>`
- `list_sessions(&self) -> Result<Vec<String>>`
- `expired_sessions(&self) -> Result<Vec<String>>`
- `prune_expired_sessions(&self) -> Result<Vec<String>>`
- `get_storage_usage(&self) -> Result<(u64, u64, usize)>`
- `check_limits(&self) -> Result<()>`
- `warn_if_near_limits(&self) -> Vec<String>`

### `Provenance` (enum)
```rust
pub enum Provenance {
    Detected,      // "detected"
    Inferred,      // "inferred"
    AiGenerated,   // "ai_generated"
    DeveloperConfirmed, // "developer_confirmed"
}
```

### `MemoryEntry`
```rust
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
```

### `McpServer`
```rust
pub struct McpServer { /* ... */ }
```

#### Methods
- `McpServer::new(project: Project, config: Config) -> Result<Self>`
- `process_request(&self, req: &JsonRpcRequest) -> JsonRpcResponse`
- `run_stdio(&self) -> Result<()>` — Run stdio MCP server loop

#### MCP Tools
| Tool | Parameters | Description |
|------|-----------|-------------|
| `project_overview` | none | Project structure overview |
| `search` | `query` | Full-text search |
| `file_context` | `path` | File metadata and symbols |
| `symbol_context` | `symbol` | Symbol definition and references |
| `dependency_context` | `path`, `transitive` | File dependencies |
| `impact_analysis` | `path` | Files affected by changes |
| `memory_context` | `scope`, `key`, `session_id` | Project/session memory |
| `session_context` | `session_id`, `action` | Session memory operations |
| `git_context` | `path`, `include_diff` | Git repository context |
| `diagnostics` | none | Health checks |
| `run_command` | `command`, `args`, `skip_confirmation` | Controlled terminal execution |

### `Exporter` / `Importer`
```rust
pub struct Exporter;
impl Exporter {
    pub fn export(project, idx, options, output_path) -> Result<ExportResult>;
}

pub struct Importer;
impl Importer {
    pub fn import(archive_path, project, options) -> Result<ImportResult>;
    pub fn verify_archive(path) -> Result<bool>;
    pub fn compute_file_hash(path) -> Result<String>;
    pub fn detect_changes(project, idx, imported_files) -> Result<Vec<ChangedFile>>;
}
```

### `Migrator`
```rust
pub struct Migrator;
```

#### Methods
- `Migrator::migrate(conn: &Connection) -> Result<()>` — Apply all pending migrations
- `Migrator::current_version(conn: &Connection) -> Result<u32>` — Get current schema version
- `Migrator::migrations() -> Vec<MigrationVersion>` — List all migrations
- `Migrator::version_count() -> u32` — Number of migrations
- `Migrator::version_compare(a: u32, b: u32) -> Ordering` — Compare versions

## CLI Commands

### Global Options
```
--help    Show help
--version Show version
```

### Command Reference
```text
cis init                          Initialize C.I.S. project
cis doctor                        Run diagnostic checks
cis status                        Show index statistics
cis scan [--help]                 Scan and index files
cis parse <path>                  Parse a single source file
cis search <query>                Full-text search
cis symbol <name>                 Find symbol definitions
cis deps <path> [--transitive]    Show dependency graph
cis impact <path>                 Show affected files
cis entrypoints                   Show entry points
cis cycles                        Check for circular dependencies
cis config <show|set>             Show/modify configuration
cis run [--yes] <command> [args]  Execute terminal command
cis git <subcommand>              Git integration
cis memory <subcommand>           Memory management
cis context <export|import|verify> Context export/import
cis mcp                           Start MCP server
```
