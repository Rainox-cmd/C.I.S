use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::process;

use crate::{analysis, config, context, diagnostics, git, index, memory, mcp, parser, project, release, scanner, terminal};

#[derive(Parser)]
#[command(name = "cis")]
#[command(about = "C.I.S. - Codebase Intelligence System")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new C.I.S. project
    Init,
    /// Run diagnostic checks on the C.I.S. installation and project
    Doctor,
    /// Show project index statistics
    Status,
    /// Scan project files and index them
    Scan {
        #[command(flatten)]
        options: ScanOptions,
    },
    /// Parse a source file and extract symbols
    Parse { path: String },
    /// Search indexed files and symbols using FTS5 full-text search
    Search { query: String },
    /// Find symbol definitions by name
    Symbol { name: String },
    /// Show dependency graph for a file
    Deps {
        /// File path (relative to project root)
        path: String,
        /// Show transitive dependencies instead of direct
        #[arg(short, long)]
        transitive: bool,
    },
    /// Show files affected by changes to a file (impact analysis)
    Impact {
        /// File path (relative to project root)
        path: String,
    },
    /// Show project entry points
    EntryPoints,
    /// Check for circular dependencies
    Cycles,
    /// Show or modify configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Execute a command through the secure terminal executor
    Run {
        /// Skip confirmation for risky commands
        #[arg(short, long)]
        yes: bool,
        /// Command and arguments to execute
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "COMMAND"
        )]
        args: Vec<String>,
    },
     /// Git integration commands
    Git {
        #[command(subcommand)]
        subcommand: GitSubcommand,
    },
    /// Memory management commands
    Memory {
        #[command(subcommand)]
        subcommand: MemorySubcommand,
    },
    /// Context export/import commands
    Context {
        #[command(subcommand)]
        subcommand: ContextSubcommand,
    },
    /// MCP server mode (for AI integration)
    Mcp,
    /// Release management commands
    Release {
        #[command(subcommand)]
        subcommand: ReleaseSubcommand,
    },
    /// Run stateless analysis on the project
    Analyze {
        #[arg(long)]
        json: bool,
    },
    /// Manage persistent analysis issues
    Issues {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        update: bool,
        #[arg(long)]
        status: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value (format: <section>.<field>)
    Set { key: String, value: String },
}

#[derive(Subcommand)]
pub enum MemorySubcommand {
    /// List all project memory entries
    List,
    /// Get a memory entry (project memory)
    Get {
        /// Key of the entry
        key: String,
    },
    /// Set a project memory entry
    Set {
        /// Key for the entry
        key: String,
        /// Value for the entry
        value: String,
        /// Provenance (detected, inferred, ai_generated, developer_confirmed)
        #[arg(short, long, default_value = "detected")]
        provenance: String,
        /// Category for the entry
        #[arg(short, long, default_value = "general")]
        category: String,
        /// Tags for the entry (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// Delete a project memory entry
    Delete {
        /// Key of the entry to delete
        key: String,
    },
    /// List all session IDs
    Sessions,
    /// Session memory operations
    Session {
        #[command(subcommand)]
        subcommand: SessionSubcommand,
    },
    /// Show storage usage and limits
    Usage,
    /// Prune expired session memory (with archive before deletion)
    Prune,
}

#[derive(Subcommand)]
pub enum ReleaseSubcommand {
    /// Generate and save the project changelog
    Changelog,
    /// Validate the presence of a release build
    Validate,
}

#[derive(Subcommand)]
pub enum SessionSubcommand {
    /// List entries in a session
    List {
        /// Session ID
        session_id: String,
    },
    /// Get a session memory entry
    Get {
        /// Session ID
        session_id: String,
        /// Key of the entry
        key: String,
    },
    /// Set a session memory entry
    Set {
        /// Session ID
        session_id: String,
        /// Key for the entry
        key: String,
        /// Value for the entry
        value: String,
        /// Provenance
        #[arg(short, long, default_value = "detected")]
        provenance: String,
        /// Tags (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// Delete a session memory entry
    Delete {
        /// Session ID
        session_id: String,
        /// Key of the entry to delete
        key: String,
    },
}

#[derive(Subcommand)]
pub enum ContextSubcommand {
    /// Export project context to a portable archive
    Export {
        /// Output file path (e.g., context.zip)
        #[arg(short, long)]
        output: String,
        /// Skip project memory
        #[arg(long)]
        no_project_memory: bool,
        /// Skip session memory
        #[arg(long)]
        no_sessions: bool,
    },
    /// Import project context from an archive
    Import {
        /// Archive file path to import
        #[arg(short, long)]
        input: String,
        /// Allow overwriting existing entries
        #[arg(long)]
        allow_overwrite: bool,
        /// Skip stale reference detection
        #[arg(long)]
        skip_stale: bool,
        /// Skip hash verification
        #[arg(long)]
        skip_verify: bool,
    },
    /// Verify an archive is valid
    Verify {
        /// Archive file path
        #[arg(short, long)]
        input: String,
    },
}

#[derive(Subcommand)]
pub enum GitSubcommand {
    /// Show working tree status
    Status,
    /// Show diff of uncommitted changes
    Diff,
    /// Show staged diff
    DiffStaged,
    /// Show diff statistics (numstat)
    DiffStat,
    /// Show recent commits
    Log {
        #[arg(short, long, default_value_t = 10)]
        count: usize,
    },
    /// Show file history
    History {
        #[arg(short, long, default_value_t = 10)]
        count: usize,
        /// File path (relative to project root)
        path: String,
    },
    /// Show last commit for a file
    Blame {
        /// File path (relative to project root)
        path: String,
    },
    /// Show current branch name
    Branch,
    /// Show if working tree is dirty
    IsDirty,
}

#[derive(Parser)]
pub struct ScanOptions {
    #[arg(short, long)]
    pub incremental: bool,
    #[arg(short, long)]
    pub path: Option<String>,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            Commands::Init => {
                let project = project::Project::discover()?;
                project.init()?;
                let cfg = config::Config::load(&project)?;
                let _idx = index::Index::open(&project, &cfg)?;
                println!("Initialized C.I.S. in {}", project.root.display());
                println!(
                    "Config: {}",
                    project.cis_dir().join("config.toml").display()
                );
                println!("Database: {}", project.db_path().display());
                println!("Logs: {}", project.logs_dir.display());
                println!("Cache: {}", project.cache_dir.display());
                Ok(())
            }
            Commands::Doctor => {
                let project = project::Project::discover()?;
                let cfg = config::Config::load(&project)?;
                let idx = index::Index::open(&project, &cfg)?;

                tracing::info!("Running diagnostic checks");

                let report = diagnostics::run_all_checks(&project, &cfg, &idx);
                report.print();

                if !report.is_healthy() {
                    process::exit(1);
                }
                Ok(())
            }
            Commands::Status => {
                let project = project::Project::discover()?;
                let cfg = config::Config::load(&project)?;
                let idx = index::Index::open(&project, &cfg)?;

                println!("C.I.S. Status");
                println!("=============");
                println!("Project root: {}", project.root.display());
                println!("C.I.S. initialized: yes");
                println!(
                    "Config: {}",
                    project.cis_dir().join("config.toml").display()
                );
                println!();

                idx.status()?;
                Ok(())
            }
            Commands::Scan { options } => {
                let project = project::Project::discover()?;
                let cfg = config::Config::load(&project)?;
                let idx = index::Index::open(&project, &cfg)?;

                let scan_root = options
                    .path
                    .map(|p| project.root.join(p))
                    .unwrap_or_else(|| project.root.clone());

                if options.incremental {
                    let previous = idx.get_file_hashes()?;
                    let scanner = scanner::Scanner::new(
                        scan_root,
                        cfg.scanner.respect_gitignore,
                        cfg.scanner.respect_cisignore,
                        cfg.general.max_file_size_bytes,
                    );
                    let result = scanner.scan_incremental(&previous)?;
                    println!("Incremental scan complete");
                    println!("  Files scanned: {}", result.scanned_count());
                    println!("  Files skipped: {}", result.skipped_count());
                    println!("  Files added: {}", result.files_added);
                    println!("  Files changed: {}", result.files_changed);
                    println!("  Files deleted: {}", result.files_deleted.len());
                    println!("  Files unchanged: {}", result.files_unchanged);
                    println!("  Total size: {} bytes", result.total_size);
                    if !result.scan_errors.is_empty() {
                        println!("  Errors: {}", result.scan_errors.len());
                    }
                    idx.upsert_files(&result.files)?;

                    {
                        let git_client = git::GitClient::new(&project.root);
                        if let Ok(client) = git_client {
                            if client.is_repo() {
                                if let Ok(commit) = client.short_commit_hash() {
                                    for file in &result.files {
                                        let _ = idx.update_git_metadata(&file.rel_path, Some(&commit));
                                    }
                                }
                            }
                        }
                    }

                    let supported = [
                        "Python", "JavaScript", "TypeScript", "Rust", "Go",
                    ];
                    for file in &result.files {
                        if supported.contains(&file.language.as_str()) {
                            let file_path = project.root.join(&file.rel_path);
                            if let Ok(content) = std::fs::read_to_string(&file_path) {
                                let parse_result = parser::ParserEngine::parse_file(
                                    &file_path,
                                    &content,
                                    &file.language,
                                );

                                let symbols: Vec<(String, String, u32, u32)> = parse_result
                                    .symbols
                                    .iter()
                                    .map(|s| (s.name.clone(), format!("{:?}", s.kind), s.line, s.column))
                                    .collect();

                                if let Err(e) = idx.upsert_symbols(&file.rel_path, &symbols) {
                                    tracing::warn!("Failed to index symbols for {}: {}", file.rel_path, e);
                                }

                                let target_paths: Vec<String> = parse_result
                                    .imports
                                    .iter()
                                    .map(|i| i.path.clone())
                                    .collect();
                                if !target_paths.is_empty() {
                                    if let Err(e) = idx.upsert_dependencies(&file.rel_path, &target_paths) {
                                        tracing::warn!("Failed to index deps for {}: {}", file.rel_path, e);
                                    }
                                }

                                let edges: Vec<(String, String, String, u32)> = parse_result
                                    .imports
                                    .iter()
                                    .filter_map(|imp| {
                                        let target_file = resolve_import_to_file(&imp.path, &file.rel_path, &file.language, &project.root);
                                        if target_file.is_empty() {
                                            None
                                        } else {
                                            Some((
                                                imp.path.clone(),
                                                target_file,
                                                if imp.is_relative { "relative" } else { "import" }.to_string(),
                                                imp.line,
                                            ))
                                        }
                                    })
                                    .collect();
                                if !edges.is_empty() {
                                    if let Err(e) = idx.upsert_symbol_edges(&file.rel_path, &edges) {
                                        tracing::warn!("Failed to index edges for {}: {}", file.rel_path, e);
                                    }
                                }
                            }
                        }
                    }

                    idx.delete_files(&result.files_deleted)?;
                } else {
                    let scanner = scanner::Scanner::new(
                        scan_root,
                        cfg.scanner.respect_gitignore,
                        cfg.scanner.respect_cisignore,
                        cfg.general.max_file_size_bytes,
                    );
                    let result = scanner.scan()?;
                    println!("Scan complete");
                    println!("  Files scanned: {}", result.scanned_count());
                    println!("  Files skipped: {}", result.skipped_count());
                    println!("  Total files discovered: {}", result.total_count());
                    println!("  Total size: {} bytes", result.total_size);
                    println!();
                    println!("Language counts:");
                    for (lang, count) in &result.language_counts {
                        println!("  {}: {}", lang, count);
                    }
                    println!();
                    println!("Category counts:");
                    for (cat, count) in &result.category_counts {
                        println!("  {}: {}", cat, count);
                    }
                    if !result.scan_errors.is_empty() {
                        println!();
                        println!("Errors:");
                        for err in &result.scan_errors {
                            println!("  {}", err);
                        }
                    }
                    idx.upsert_files(&result.files)?;

                    {
                        let git_client = git::GitClient::new(&project.root);
                        if let Ok(client) = git_client {
                            if client.is_repo() {
                                if let Ok(commit) = client.short_commit_hash() {
                                    for file in &result.files {
                                        let _ = idx.update_git_metadata(&file.rel_path, Some(&commit));
                                    }
                                }
                            }
                        }
                    }

                    let supported = [
                        "Python", "JavaScript", "TypeScript", "Rust", "Go",
                    ];
                    for file in &result.files {
                        if supported.contains(&file.language.as_str()) {
                                let file_path = project.root.join(&file.rel_path);
                                if let Ok(content) = std::fs::read_to_string(&file_path) {
                                    let parse_result = parser::ParserEngine::parse_file(
                                        &file_path,
                                        &content,
                                        &file.language,
                                    );

                                    let symbols: Vec<(String, String, u32, u32)> = parse_result
                                        .symbols
                                        .iter()
                                        .map(|s| (s.name.clone(), format!("{:?}", s.kind), s.line, s.column))
                                        .collect();

                                    if let Err(e) = idx.upsert_symbols(&file.rel_path, &symbols) {
                                        tracing::warn!("Failed to index symbols for {}: {}", file.rel_path, e);
                                    }

                                    let target_paths: Vec<String> = parse_result
                                        .imports
                                        .iter()
                                        .map(|i| i.path.clone())
                                        .collect();
                                    if !target_paths.is_empty() {
                                        if let Err(e) = idx.upsert_dependencies(&file.rel_path, &target_paths) {
                                            tracing::warn!("Failed to index deps for {}: {}", file.rel_path, e);
                                        }
                                    }

                                    let edges: Vec<(String, String, String, u32)> = parse_result
                                        .imports
                                        .iter()
                                        .filter_map(|imp| {
                                            let target_file = resolve_import_to_file(&imp.path, &file.rel_path, &file.language, &project.root);
                                            if target_file.is_empty() {
                                                None
                                            } else {
                                                Some((
                                                    imp.path.clone(),
                                                    target_file,
                                                    if imp.is_relative { "relative" } else { "import" }.to_string(),
                                                    imp.line,
                                                ))
                                            }
                                        })
                                        .collect();
                                    if !edges.is_empty() {
                                        if let Err(e) = idx.upsert_symbol_edges(&file.rel_path, &edges) {
                                            tracing::warn!("Failed to index edges for {}: {}", file.rel_path, e);
                                        }
                                    }
                                }
                            }
                    }
                }
                Ok(())
              }
              Commands::Parse { path } => {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path))?;

                let ext = std::path::Path::new(&path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| format!(".{}", e.to_lowercase()))
                    .unwrap_or_default();

                let language = scanner::LANGUAGE_MAP
                    .iter()
                    .find(|(e, _)| *e == ext)
                    .map(|(_, l)| *l)
                    .unwrap_or("Other");

                let result = parser::ParserEngine::parse_file(
                    std::path::Path::new(&path),
                    &content,
                    language,
                );

                println!("Parsed: {} ({})", path, language);
                println!("Syntax OK: {}", result.syntax_ok);
                if let Some(err) = result.syntax_error {
                    println!("Syntax Error: {}", err);
                }
                println!("\nSymbols ({}):", result.symbols.len());
                for sym in &result.symbols {
                    println!("  {:?} {}:{}", sym.kind, sym.name, sym.line);
                }
                println!("\nImports ({}):", result.imports.len());
                for imp in &result.imports {
                    let rel = if imp.is_relative {
                        "relative"
                    } else {
                        "absolute"
                    };
                    println!("  {} ({})", imp.path, rel);
                }
                 Ok(())
             }
             Commands::Search { query } => {
                 let project = project::Project::discover()?;
                 let cfg = config::Config::load(&project)?;
                 let idx = index::Index::open(&project, &cfg)?;

                 let results = idx.search(&query)?;
                 if results.is_empty() {
                     println!("No results found for '{}'", query);
                 } else {
                     println!("Search results for '{}':", query);
                     for r in &results {
                         println!(
                             "  {} ({}) - {}:{} (rank: {:.2})",
                             r.name, r.symbol_type, r.rel_path, r.line, r.rank
                         );
                     }
                     println!("\n{} result(s)", results.len());
                 }
                 Ok(())
             }
              Commands::Symbol { name } => {
                  let project = project::Project::discover()?;
                  let cfg = config::Config::load(&project)?;
                  let idx = index::Index::open(&project, &cfg)?;

                  let symbols = idx.find_symbols_by_name(&name)?;
                  if symbols.is_empty() {
                      println!("No symbols found matching '{}'", name);
                  } else {
                      println!("Symbol definitions for '{}':", name);
                      for s in &symbols {
                          println!(
                              "  {} {}:{} ({} - {})",
                              s.symbol_type, s.rel_path, s.line, s.name, s.symbol_type
                          );
                      }
                      println!("\n{} result(s)", symbols.len());
                  }
                  Ok(())
              }
              Commands::Deps { path, transitive } => {
                  let project = project::Project::discover()?;
                  let cfg = config::Config::load(&project)?;
                  let idx = index::Index::open(&project, &cfg)?;

                  if transitive {
                      let deps = idx.get_transitive_dependencies(&path)?;
                      if deps.is_empty() {
                          println!("No transitive dependencies found for '{}'", path);
                      } else {
                          println!("Transitive dependencies of '{}':", path);
                          for d in &deps {
                              println!("  {}", d);
                          }
                          println!("\n{} result(s)", deps.len());
                      }
                  } else {
                      let deps = idx.get_dependencies(&path)?;
                      if deps.is_empty() {
                          println!("No direct dependencies found for '{}'", path);
                      } else {
                          println!("Direct dependencies of '{}':", path);
                          for d in &deps {
                              println!("  {}", d.target_file);
                          }
                          println!("\n{} result(s)", deps.len());
                      }
                  }
                  Ok(())
              }
              Commands::Impact { path } => {
                  let project = project::Project::discover()?;
                  let cfg = config::Config::load(&project)?;
                  let idx = index::Index::open(&project, &cfg)?;

                  let affected = idx.get_reverse_dependencies(&path)?;
                  if affected.is_empty() {
                      println!("No files affected by changes to '{}'", path);
                  } else {
                      println!("Files affected by changes to '{}':", path);
                      for a in &affected {
                          println!("  {}", a);
                      }
                      println!("\n{} result(s)", affected.len());
                  }
                  Ok(())
              }
              Commands::EntryPoints => {
                  let project = project::Project::discover()?;
                  let cfg = config::Config::load(&project)?;
                  let idx = index::Index::open(&project, &cfg)?;

                  let entries = idx.get_entry_points()?;
                  if entries.is_empty() {
                      println!("No entry points found");
                  } else {
                      println!("Entry points (no incoming dependencies):");
                      for e in entries.iter().filter(|e| e.incoming_count == 0) {
                          println!("  {} ({} symbols)", e.rel_path, e.symbol_count);
                      }
                      println!();
                      println!("All source files:");
                      for e in &entries {
                          println!("  {} ({} symbols, {} incoming)", e.rel_path, e.symbol_count, e.incoming_count);
                      }
                      println!("\n{} total source files", entries.len());
                  }
                  Ok(())
              }
              Commands::Cycles => {
                  let project = project::Project::discover()?;
                  let cfg = config::Config::load(&project)?;
                  let idx = index::Index::open(&project, &cfg)?;

                  let has_cycle = idx.has_cycle()?;
                  if has_cycle {
                      println!("Circular dependencies detected!");
                  } else {
                      println!("No circular dependencies found.");
                  }
                  Ok(())
              }
              Commands::Config { action } => {
                let project = project::Project::discover()?;
                let mut cfg = config::Config::load(&project)?;
                match action {
                    ConfigAction::Show => {
                        println!("{}", toml::to_string_pretty(&cfg)?);
                        Ok(())
                    }
                    ConfigAction::Set { key, value } => {
                        cfg.set(&key, &value)?;
                        cfg.save(&project)?;
                        println!("Set {} = {}", key, value);
                        Ok(())
                    }
                }
            }
              Commands::Git { subcommand } => {
                  let project = project::Project::discover()?;
                  let client = git::GitClient::new(&project.root)?;

                  match subcommand {
                      GitSubcommand::Status => {
                          if !client.is_repo() {
                              println!("Not a Git repository");
                              return Ok(());
                          }
                          println!("Branch: {}", client.current_branch()?);
                          println!("Dirty: {}", client.is_dirty()?);

                          if let Ok(hash) = client.short_commit_hash() {
                              if !hash.is_empty() {
                                  println!("HEAD: {}", hash);
                              }
                          }

                          let entries = client.status()?;
                          if entries.is_empty() {
                              println!("Working tree clean");
                          } else {
                              println!("\nChanges:");
                              for e in &entries {
                                  println!("  {} {}", e.status, e.path);
                              }
                          }
                          Ok(())
                      }
                      GitSubcommand::Diff => {
                          if !client.is_repo() {
                              println!("Not a Git repository");
                              return Ok(());
                          }
                          let diff = client.diff()?;
                          if diff.is_empty() {
                              println!("No uncommitted changes");
                          } else {
                              println!("{}", diff);
                          }
                          Ok(())
                      }
                      GitSubcommand::DiffStaged => {
                          if !client.is_repo() {
                              println!("Not a Git repository");
                              return Ok(());
                          }
                          let diff = client.diff_staged()?;
                          if diff.is_empty() {
                              println!("No staged changes");
                          } else {
                              println!("{}", diff);
                          }
                          Ok(())
                      }
                      GitSubcommand::DiffStat => {
                          if !client.is_repo() {
                              println!("Not a Git repository");
                              return Ok(());
                          }
                          let entries = client.diff_stat()?;
                          if entries.is_empty() {
                              println!("No uncommitted changes");
                          } else {
                              println!("Diff statistics:");
                              for e in &entries {
                                  println!(
                                      "  {:>6} {:<6} {} ({})",
                                      format!("+{}", e.additions),
                                      format!("-{}", e.deletions),
                                      e.path,
                                      e.status
                                  );
                              }
                              println!("\n{} file(s) changed", entries.len());
                          }
                          Ok(())
                      }
                      GitSubcommand::Log { count } => {
                          if !client.is_repo() {
                              println!("Not a Git repository");
                              return Ok(());
                          }
                          let commits = client.log(count)?;
                          if commits.is_empty() {
                              println!("No commits found");
                          } else {
                              println!("Recent commits:");
                              for c in &commits {
                                  println!("  {} {} <{}> - {}", c.short_hash, c.author, c.author_email, c.message);
                                  println!("    {}", c.date);
                              }
                              println!("\n{} commit(s)", commits.len());
                          }
                          Ok(())
                      }
                      GitSubcommand::History { count, path } => {
                          if !client.is_repo() {
                              println!("Not a Git repository");
                              return Ok(());
                          }
                          let commits = client.file_history(&path, count)?;
                          if commits.is_empty() {
                              println!("No history for '{}'", path);
                          } else {
                              println!("History for '{}':", path);
                              for c in &commits {
                                  println!("  {} {} <{}> - {}", c.short_hash, c.author, c.author_email, c.message);
                                  println!("    {}", c.date);
                              }
                              println!("\n{} commit(s)", commits.len());
                          }
                          Ok(())
                      }
                      GitSubcommand::Blame { path } => {
                          if !client.is_repo() {
                              println!("Not a Git repository");
                              return Ok(());
                          }
                          match client.file_last_commit(&path)? {
                              Some(c) => {
                                  println!("Path: {}", path);
                                  println!("  Last commit: {} by {} <{}>", c.short_hash, c.author, c.author_email);
                                  println!("  Date: {}", c.date);
                                  println!("  Message: {}", c.message);
                              }
                              None => {
                                  println!("No Git history for '{}'", path);
                              }
                          }
                          Ok(())
                      }
                      GitSubcommand::Branch => {
                          if !client.is_repo() {
                              println!("Not a Git repository");
                              return Ok(());
                          }
                          println!("{}", client.current_branch()?);
                          Ok(())
                      }
                      GitSubcommand::IsDirty => {
                          if !client.is_repo() {
                              println!("Not a Git repository");
                              return Ok(());
                          }
                          let dirty = client.is_dirty()?;
                          if dirty {
                              println!("Working tree is dirty");
                          } else {
                              println!("Working tree is clean");
                          }
                          Ok(())
                      }
                  }
              }
              Commands::Memory { subcommand } => {
                  let project = project::Project::discover()?;
                  let cfg = config::Config::load(&project)?;
                  let mem = memory::MemoryManager::new(&project, &cfg.context)?;

                  match subcommand {
                      MemorySubcommand::List => {
                          let entries = mem.project_list()?;
                          if entries.is_empty() {
                              println!("No project memory entries");
                          } else {
                              println!("Project memory ({} entries):", entries.len());
                              for e in &entries {
                                  println!(
                                      "  {} = {} [{}, {}] tags: [{}]",
                                      e.key, e.value, e.provenance, e.category,
                                      e.tags.join(", ")
                                  );
                              }
                          }
                          Ok(())
                      }
                      MemorySubcommand::Get { key } => {
                          match mem.project_get(&key)? {
                              Some(e) => {
                                  println!("Key: {}", e.key);
                                  println!("Value: {}", e.value);
                                  println!("Provenance: {}", e.provenance);
                                  println!("Category: {}", e.category);
                                  println!("Created: {}", e.created_at);
                                  println!("Updated: {}", e.updated_at);
                                  if let Some(expires) = e.expires_at {
                                      println!("Expires: {}", expires);
                                  }
                                  if !e.tags.is_empty() {
                                      println!("Tags: {}", e.tags.join(", "));
                                  }
                              }
                              None => {
                                  println!("No memory entry found for key: '{}'", key);
                              }
                          }
                          Ok(())
                      }
                      MemorySubcommand::Set { key, value, provenance, category, tags } => {
                          let prov: memory::Provenance = provenance.parse()?;
                          mem.project_set(&key, &value, prov, &category, None, tags)?;
                          println!("Set '{}' = '{}' (provenance: {}, category: {})", key, value, prov, category);
                          Ok(())
                      }
                      MemorySubcommand::Delete { key } => {
                          let deleted = mem.project_delete(&key)?;
                          if deleted {
                              println!("Deleted memory entry: '{}'", key);
                          } else {
                              println!("No memory entry found for key: '{}'", key);
                          }
                          Ok(())
                      }
                      MemorySubcommand::Sessions => {
                          let sessions = mem.list_sessions()?;
                          if sessions.is_empty() {
                              println!("No sessions found");
                          } else {
                              println!("Sessions ({}):", sessions.len());
                              for s in &sessions {
                                  println!("  session-{}", s);
                              }
                          }
                          Ok(())
                      }
                      MemorySubcommand::Session { subcommand } => {
                          match subcommand {
                              SessionSubcommand::List { session_id } => {
                                  let entries = mem.session_list(&session_id)?;
                                  if entries.is_empty() {
                                      println!("No entries in session: {}", session_id);
                                  } else {
                                      println!("Session '{}' ({} entries):", session_id, entries.len());
                                      for e in &entries {
                                          println!("  {} = {}", e.key, e.value);
                                      }
                                  }
                                  Ok(())
                              }
                              SessionSubcommand::Get { session_id, key } => {
                                  match mem.session_get(&session_id, &key)? {
                                      Some(e) => {
                                          println!("Key: {}", e.key);
                                          println!("Value: {}", e.value);
                                          println!("Provenance: {}", e.provenance);
                                      }
                                      None => {
                                          println!("No entry found for session '{}' key '{}'", session_id, key);
                                      }
                                  }
                                  Ok(())
                              }
                              SessionSubcommand::Set { session_id, key, value, provenance, tags } => {
                                  let prov: memory::Provenance = provenance.parse()?;
                                  mem.session_set(&session_id, &key, &value, prov, None, tags)?;
                                  println!("Set session '{}' key '{}' = '{}'", session_id, key, value);
                                  Ok(())
                              }
                              SessionSubcommand::Delete { session_id, key } => {
                                  let deleted = mem.session_delete(&session_id, &key)?;
                                  if deleted {
                                      println!("Deleted session '{}' key '{}'", session_id, key);
                                  } else {
                                      println!("No entry found for session '{}' key '{}'", session_id, key);
                                  }
                                  Ok(())
                              }
                          }
                      }
                      MemorySubcommand::Usage => {
                          let (project_size, total_size, session_count) = mem.get_storage_usage()?;
                          println!("Memory usage:");
                          println!("  Project memory: {} bytes", project_size);
                          println!("  Total context: {} bytes", total_size);
                          println!("  Sessions: {}", session_count);
                          println!();
                          println!("Limits:");
                          println!("  Per-session: {} bytes", cfg.context.max_session_context_bytes);
                          println!("  Max sessions: {}", cfg.context.max_sessions);
                          println!("  Total storage: {} bytes", cfg.context.max_total_context_bytes);
                          println!("  Max file size: {} bytes", cfg.context.max_context_file_bytes);
                          println!("  Session TTL: {} seconds", cfg.context.session_ttl_seconds);

                          let warnings = mem.warn_if_near_limits();
                          if !warnings.is_empty() {
                              println!();
                              println!("Warnings:");
                              for w in &warnings {
                                  println!("  {}", w);
                              }
                          }
                          Ok(())
                      }
                      MemorySubcommand::Prune => {
                          let expired = mem.expired_sessions()?;
                          if expired.is_empty() {
                              println!("No expired sessions to prune");
                          } else {
                              println!("Expired sessions (will be archived before deletion):");
                              for s in &expired {
                                  println!("  session-{}", s);
                              }
                              println!();
                              println!("{} session(s) will be pruned", expired.len());
                              let pruned = mem.prune_expired_sessions()?;
                              println!("Pruned {} session(s) (archived to .cis/cache/deleted_sessions/)", pruned.len());
                          }
                          Ok(())
                      }
                  }
              }
              Commands::Mcp => {
                  let project = project::Project::discover()?;
                  let cfg = config::Config::load(&project)?;
                  let server = mcp::McpServer::new(project, cfg)?;
                  server.run_stdio()?;
                  Ok(())
              }
              Commands::Release { subcommand } => {
                  let project = project::Project::discover()?;
                  match subcommand {
                      ReleaseSubcommand::Changelog => {
                          release::ReleaseManager::save_changelog(&project)?;
                          println!(
                              "Changelog successfully generated at {}",
                              release::ReleaseManager::changelog_path(&project).display()
                          );
                      }
                      ReleaseSubcommand::Validate => {
                          let is_valid = release::ReleaseManager::validate_build(&project)?;
                          if is_valid {
                              println!("Build validation passed.");
                          } else {
                              println!("Build validation failed. Release binary not found.");
                              process::exit(1);
                          }
                      }
                  }
                  Ok(())
              }
              Commands::Analyze { json } => {
                  let project = project::Project::discover()?;
                  let cfg = config::Config::load(&project)?;
                  let idx = index::Index::open(&project, &cfg)?;
                  let report = crate::analysis::AnalysisEngine::run(&project, &cfg, &idx)?;

                  if json {
                      println!("{}", serde_json::to_string_pretty(&report)?);
                  } else {
                      println!("C.I.S. Analysis");
                      println!("===============");
                      println!("\nProject: {}", report.project_path);
                      println!("\nIssues: {}", report.issues.len());

                      if report.issues.is_empty() {
                          println!("\nNo issues found.");
                      } else {
                          let mut scan_errors = Vec::new();
                          let mut syntax_errors = Vec::new();
                          let mut cycles = Vec::new();

                          for issue in &report.issues {
                              match issue.category {
                                  crate::analysis::IssueCategory::ScanError => scan_errors.push(issue),
                                  crate::analysis::IssueCategory::SyntaxError => syntax_errors.push(issue),
                                  crate::analysis::IssueCategory::DependencyCycle => cycles.push(issue),
                              }
                          }

                          if !scan_errors.is_empty() {
                              println!("\nScan Errors");
                              println!("-----------");
                              for err in scan_errors {
                                  println!("- {}", err.message);
                              }
                          }

                          if !syntax_errors.is_empty() {
                              println!("\nSyntax Errors");
                              println!("-------------");
                              for err in syntax_errors {
                                  println!("- {}: {}", err.file.as_deref().unwrap_or("unknown"), err.message);
                              }
                          }

                          if !cycles.is_empty() {
                              println!("\nDependency Cycles");
                              println!("-----------------");
                              for cycle in cycles {
                                  println!("- {}", cycle.message);
                              }
                          }
                      }

                      if !report.informational.is_empty() {
                          println!("\nInformational");
                          println!("-------------");
                          for info in &report.informational {
                              println!("{}", info);
                          }
                      }
                  }
                  Ok(())
              }
              Commands::Issues { json, update, status } => {
                  let project = project::Project::discover()?;
                  let cfg = config::Config::load(&project)?;
                  let idx = index::Index::open(&project, &cfg)?;

                  if update {
                      let report = crate::analysis::AnalysisEngine::run(&project, &cfg, &idx)?;
                      let changed = idx.save_issues(&report.issues)?;
                      if !json {
                          println!("Analysis complete. Reconciled issues ({} changes).", changed);
                      }
                  }

                  let issues = idx.get_issues(status.as_deref())?;

                  if json {
                      println!("{}", serde_json::to_string_pretty(&issues)?);
                  } else {
                      println!("C.I.S. Issues");
                      println!("=============");
                      if issues.is_empty() {
                          println!("No issues found.");
                      } else {
                          for issue in issues {
                              let file_display = issue.file.as_deref().unwrap_or("general");
                              println!(
                                  "[{}] {} | {:?} | {}: {}",
                                  issue.id, issue.status.to_uppercase(), issue.severity, file_display, issue.message
                              );
                          }
                      }
                  }
                  Ok(())
              }
              Commands::Context { subcommand } => {
                  let project = project::Project::discover()?;
                  let cfg = config::Config::load(&project)?;
                  let idx = index::Index::open(&project, &cfg)?;

                  match subcommand {
                      ContextSubcommand::Export { output, no_project_memory, no_sessions } => {
                          let options = context::ExportOptions {
                              include_project_memory: !no_project_memory,
                              include_sessions: !no_sessions,
                          };
                          let output_path = std::path::PathBuf::from(&output);
                          let result = context::Exporter::export(&project, &idx, &options, &output_path)?;
                          println!("Exported context to: {}", result.archive_path);
                          println!("  Files: {}", result.files_exported);
                          println!("  Sessions: {}", result.sessions_exported);
                          println!("  Project memory entries: {}", result.project_memory_entries);
                          println!("  Archive size: {} bytes", result.archive_size);
                          Ok(())
                      }
                      ContextSubcommand::Import { input, allow_overwrite, skip_stale, skip_verify } => {
                          let options = context::ImportOptions {
                              allow_overwrite,
                              skip_stale,
                              verify_hashes: !skip_verify,
                          };
                          let input_path = std::path::PathBuf::from(&input);
                          if !input_path.exists() {
                              anyhow::bail!("Archive not found: {}", input);
                          }
                          let result = context::Importer::import(&input_path, &project, &options)?;
                          println!("Import results:");
                          println!("  Files imported: {}", result.files_imported);
                          println!("  Sessions imported: {}", result.sessions_imported);
                          println!("  Memory entries imported: {}", result.project_memory_entries);

                          if !result.stale_references.is_empty() {
                              println!("\nStale references (not silently ignored):");
                              for r in &result.stale_references {
                                  println!("  {} ({})", r.rel_path, r.kind);
                              }
                          }

                          if !result.changed_files.is_empty() {
                              println!("\nChanged files:");
                              for c in &result.changed_files {
                                  println!("  {} ({})", c.rel_path, c.status);
                              }
                          }

                          if !result.warnings.is_empty() {
                              println!("\nWarnings:");
                              for w in &result.warnings {
                                  println!("  {}", w);
                              }
                          }
                          Ok(())
                      }
                      ContextSubcommand::Verify { input } => {
                          let input_path = std::path::PathBuf::from(&input);
                          if !input_path.exists() {
                              anyhow::bail!("Archive not found: {}", input);
                          }
                          let valid = context::Importer::verify_archive(&input_path)?;
                          if valid {
                              println!("Archive is valid: {}", input);
                          } else {
                              println!("Archive is invalid (missing manifest): {}", input);
                          }
                          Ok(())
                      }
                  }
              }
              Commands::Run { yes, args } => {
                if args.is_empty() {
                    anyhow::bail!(
                        "No command specified. Usage: cis run [--yes] <command> [args...]"
                    );
                }

                let project = project::Project::discover()?;
                let cfg = config::Config::load(&project)?;

                let executor = terminal::TerminalExecutor::from_config(
                    project.root.clone(),
                    project.logs_dir.clone(),
                    &cfg,
                );

                let command = &args[0];
                let cmd_args: Vec<&str> = args[1..].iter().map(String::as_str).collect();

                let result = executor.execute(command, &cmd_args, None, yes)?;

                if result.requires_confirmation {
                    println!(
                        "Risky command requires confirmation: {}",
                        result.risk_description.as_deref().unwrap_or("unknown risk")
                    );
                    println!("Review the command and rerun with --yes to proceed.");
                    process::exit(2);
                }

                if result.is_blocked() {
                    println!("Command blocked by security policy: {}", result.stderr);
                    process::exit(1);
                }

                if result.is_timed_out() {
                    println!(
                        "Command timed out after {} seconds",
                        cfg.security.default_timeout_seconds
                    );
                    process::exit(124);
                }

                if !result.stdout.is_empty() {
                    print!("{}", result.stdout);
                }
                if !result.stderr.is_empty() {
                    eprint!("{}", result.stderr);
                }

                if result.exit_code != 0 {
                    process::exit(result.exit_code);
                }

                Ok(())
            }
        }
    }
}

fn resolve_import_to_file(imp_path: &str, source_path: &str, language: &str, project_root: &std::path::Path) -> String {
    if imp_path.is_empty() || imp_path == "." {
        return String::new();
    }

    let normalized = imp_path.replace('.', "/");

    let candidates: Vec<String> = match language {
        "Python" => vec![
            format!("{}.py", normalized),
            format!("{}/__init__.py", normalized),
        ],
        "Rust" => vec![
            format!("{}.rs", normalized),
            format!("{}/mod.rs", normalized),
            format!("{}/main.rs", normalized),
            format!("{}/lib.rs", normalized),
        ],
        "Go" => vec![
            format!("{}.go", normalized),
        ],
        "JavaScript" | "TypeScript" => vec![
            format!("{}.js", normalized),
            format!("{}.jsx", normalized),
            format!("{}.ts", normalized),
            format!("{}.tsx", normalized),
            format!("{}/index.js", normalized),
            format!("{}/index.ts", normalized),
        ],
        _ => vec![format!("{}.{}", normalized, language.to_lowercase())],
    };

    for candidate in candidates {
        if project_root.join(&candidate).exists() {
            return candidate;
        }
    }

    if source_path == imp_path {
        return imp_path.to_string();
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_import_to_file_uses_project_root() {
        let dir = tempdir().unwrap();
        let project_root = dir.path();
        
        // Create a fake target structure inside the temp dir
        let module_dir = project_root.join("ai");
        fs::create_dir_all(&module_dir).unwrap();
        fs::write(module_dir.join("client.py"), "print('hello')").unwrap();
        
        // Test that it resolves correctly using project_root, even if CWD is different
        let resolved = resolve_import_to_file("ai.client", "main.py", "Python", project_root);
        assert_eq!(resolved, "ai/client.py");
        
        // Test absolute imports
        let missing = resolve_import_to_file("ai.missing", "main.py", "Python", project_root);
        assert_eq!(missing, "");
    }

    #[test]
    fn test_analyze_cli_parsing() {
        // Test 1: cis analyze
        let cli = Cli::try_parse_from(vec!["cis", "analyze"]).unwrap();
        match cli.command {
            Commands::Analyze { json } => assert!(!json, "json should be false"),
            _ => panic!("Expected Commands::Analyze"),
        }

        // Test 2: cis analyze --json
        let cli_json = Cli::try_parse_from(vec!["cis", "analyze", "--json"]).unwrap();
        match cli_json.command {
            Commands::Analyze { json } => assert!(json, "json should be true"),
            _ => panic!("Expected Commands::Analyze"),
        }
    }

    #[test]
    fn test_issues_cli_parsing() {
        let cli = Cli::try_parse_from(vec!["cis", "issues"]).unwrap();
        match cli.command {
            Commands::Issues { json, update, status } => {
                assert!(!json);
                assert!(!update);
                assert_eq!(status, None);
            }
            _ => panic!("Expected Commands::Issues"),
        }

        let cli2 = Cli::try_parse_from(vec!["cis", "issues", "--update", "--json", "--status", "open"]).unwrap();
        match cli2.command {
            Commands::Issues { json, update, status } => {
                assert!(json);
                assert!(update);
                assert_eq!(status.as_deref(), Some("open"));
            }
            _ => panic!("Expected Commands::Issues"),
        }
    }
}