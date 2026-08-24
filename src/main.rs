mod cli;
mod config;
mod index;
mod parser;
mod project;
mod scanner;

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
