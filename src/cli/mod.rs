use clap::{Parser, Subcommand};
use anyhow::{Context, Result};

use crate::{config, index, parser, project, scanner};

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
    Init,
    Doctor,
    Status,
    Scan {
        #[command(flatten)]
        options: ScanOptions,
    },
    Parse {
        path: String,
    },
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    Show,
    Set {
        key: String,
        value: String,
    },
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
                println!("Config: {}", project.cis_dir().join("config.toml").display());
                println!("Database: {}", project.db_path().display());
                Ok(())
            }
            Commands::Doctor => {
                let project = project::Project::discover()?;
                let cfg = config::Config::load(&project)?;
                let idx = index::Index::open(&project, &cfg)?;
                idx.doctor()?;
                Ok(())
            }
            Commands::Status => {
                let project = project::Project::discover()?;
                let cfg = config::Config::load(&project)?;
                let idx = index::Index::open(&project, &cfg)?;
                idx.status()?;
                Ok(())
            }
            Commands::Scan { options } => {
                let project = project::Project::discover()?;
                let cfg = config::Config::load(&project)?;
                let idx = index::Index::open(&project, &cfg)?;
                
                let scan_root = options.path
                    .map(|p| project.root.join(p))
                    .unwrap_or_else(|| project.root.clone());

                if options.incremental {
                    let previous = idx.get_file_hashes()?;
                    let scanner = scanner::Scanner::new(scan_root, cfg.scanner.respect_gitignore, cfg.scanner.respect_cisignore);
                    let result = scanner.scan_incremental(&previous)?;
                    println!("Incremental scan complete: {} files", result.files.len());
                    idx.upsert_files(&result.files)?;
                } else {
                    let scanner = scanner::Scanner::new(scan_root, cfg.scanner.respect_gitignore, cfg.scanner.respect_cisignore);
                    let result = scanner.scan()?;
                    println!("Scan complete: {} files", result.files.len());
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

                let language = scanner::LANGUAGE_MAP.iter()
                    .find(|(e, _)| *e == ext)
                    .map(|(_, l)| *l)
                    .unwrap_or("Other");

                let result = parser::ParserEngine::parse_file(std::path::Path::new(&path), &content, language);
                
                println!("Parsed: {} ({})", path, language);
                println!("Syntax OK: {}", result.syntax_ok);
                if let Some(err) = result.syntax_error {
                    println!("Syntax Error: {}", err);
                }
                println!("\nSymbols ({}):", result.symbols.len());
                for sym in &result.symbols {
                    println!("  {} {}:{}", format!("{:?}", sym.kind), sym.name, sym.line);
                }
                println!("\nImports ({}):", result.imports.len());
                for imp in &result.imports {
                    println!("  {} ({})", imp.path, if imp.is_relative { "relative" } else { "absolute" });
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
        }
    }
}
