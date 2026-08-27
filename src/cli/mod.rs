use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::process;

use crate::{config, diagnostics, git, index, parser, project, scanner, terminal};

#[derive(Parser)]
#[command(name = "cis")]
#[command(about = "C.I.S. - Codebase Intelligence System")]
#[command(version = "0.1.0")]
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
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value (format: <section>.<field>)
    Set { key: String, value: String },
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
                                        let target_file = resolve_import_to_file(&imp.path, &file.rel_path, &file.language);
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
                                            let target_file = resolve_import_to_file(&imp.path, &file.rel_path, &file.language);
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
              Commands::Git { subcommand } => {
                  let project = project::Project::discover()?;
                  let client = git::GitClient::new(&project.root)?;

                  if !client.is_repo() {
                      println!("Not a Git repository: {}", project.root.display());
                      return Ok(());
                  }

                  match subcommand {
                      GitSubcommand::Status => {
                          let entries = client.status()?;
                          println!("Branch: {}", client.current_branch()?);
                          println!("Dirty: {}", client.is_dirty()?);
                          println!();
                          if entries.is_empty() {
                              println!("Working tree clean");
                          } else {
                              println!("Changes:");
                              for e in &entries {
                                  println!("  {} {}", e.status, e.path);
                              }
                              println!("\n{} file(s) changed", entries.len());
                          }
                          Ok(())
                      }
                      GitSubcommand::Diff => {
                          let diff = client.diff()?;
                          if diff.is_empty() {
                              println!("No uncommitted changes");
                          } else {
                              println!("{}", diff);
                          }
                          Ok(())
                      }
                      GitSubcommand::DiffStaged => {
                          let diff = client.diff_staged()?;
                          if diff.is_empty() {
                              println!("No staged changes");
                          } else {
                              println!("{}", diff);
                          }
                          Ok(())
                      }
                      GitSubcommand::DiffStat => {
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
                          match client.file_last_commit(&path)? {
                              Some(c) => {
                                  println!("{}: last modified by {} <{}> at {}", path, c.author, c.author_email, c.date);
                                  println!("  commit {} - {}", c.short_hash, c.message);
                              }
                              None => {
                                  println!("No Git history for '{}'", path);
                              }
                          }
                          Ok(())
                      }
                      GitSubcommand::Branch => {
                          println!("{}", client.current_branch()?);
                          Ok(())
                      }
                      GitSubcommand::IsDirty => {
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

fn resolve_import_to_file(imp_path: &str, source_path: &str, language: &str) -> String {
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
        if std::path::Path::new(&candidate).exists() {
            return candidate;
        }
    }

    if source_path == imp_path {
        return imp_path.to_string();
    }

    String::new()
}