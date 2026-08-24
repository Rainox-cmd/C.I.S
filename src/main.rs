#[allow(dead_code, unused_imports)]
mod cli;
mod config;
#[allow(dead_code)]
mod diagnostics;
#[allow(dead_code)]
mod index;
mod parser;
mod project;
mod scanner;
#[allow(dead_code, unused_imports)]
mod security;
#[allow(dead_code, unused_imports)]
mod terminal;

use clap::Parser;
use anyhow::Result;
use tracing::info;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    info!("C.I.S. starting up");

    let cli = cli::Cli::parse();
    cli.run()?;

    Ok(())
}
