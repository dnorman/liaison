use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod config;
mod discovery;
mod html;
mod plaintext;
mod processor;
mod resolver;

#[derive(Parser)]
#[command(name = "liaison")]
#[command(about = "Materialize referenced content into source files in place")]
struct Cli {
    /// Dry run - check if changes would be made
    #[arg(long)]
    check: bool,

    /// Files to process
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let repo_root = resolver::find_repo_root()?;
    let config = config::Config::load(&repo_root)?;
    let files = discovery::discover_files(&repo_root, &config, &cli.paths)?;

    let changes = processor::process_files(&repo_root, &files)?;

    if cli.check {
        if changes.is_empty() {
            std::process::exit(0);
        } else {
            eprintln!("Changes would be made to {} file(s)", changes.len());
            std::process::exit(1);
        }
    } else {
        processor::apply_changes(&changes)?;
        eprintln!("Updated {} file(s)", changes.len());
    }

    Ok(())
}
