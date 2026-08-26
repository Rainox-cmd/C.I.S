use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::process;

use crate::{config, diagnostics, index, parser, project, scanner, terminal};

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
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value (format: <section>.<field>)
    Set { key: String, value: String },
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
