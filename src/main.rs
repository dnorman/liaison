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

    /// Reset all transclude blocks to empty content
    #[arg(long)]
    reset: bool,

    /// Files to process
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Validate that all paths exist
    for path in &cli.paths {
        if !path.exists() {
            anyhow::bail!("Path does not exist: {}", path.display());
        }
        if !path.is_file() {
            anyhow::bail!("Path is not a file: {}", path.display());
        }
    }

    // Determine repo root based on the first file if specified, otherwise use CWD
    let repo_root = if let Some(first_file) = cli.paths.first() {
        let root = resolver::find_repo_root_for_path(first_file)?;

        // Validate that all specified files are in the same repository
        for file in &cli.paths[1..] {
            let file_root = resolver::find_repo_root_for_path(file)?;
            if file_root != root {
                anyhow::bail!(
                    "All files must be in the same repository.\n  {} is in {}\n  {} is in {}",
                    first_file.display(),
                    root.display(),
                    file.display(),
                    file_root.display()
                );
            }
        }

        root
    } else {
        resolver::find_repo_root()?
    };

    let config = config::Config::load(&repo_root)?;
    let files = discovery::discover_files(&repo_root, &config, &cli.paths)?;

    if cli.reset {
        let changes = processor::reset_files(&files)?;
        processor::apply_changes(&changes)?;
        eprintln!("Reset {} file(s)", changes.len());
    } else {
        let result = processor::process_files(&repo_root, &files)?;

        result.dependencies.print_tree(&files, &repo_root);
        eprintln!();

        if cli.check {
            if result.changes.is_empty() {
                eprintln!("No changes needed");
                std::process::exit(0);
            } else {
                eprintln!("Changes would be made to {} file(s)", result.changes.len());
                std::process::exit(1);
            }
        } else {
            processor::apply_changes(&result.changes)?;
            eprintln!("Updated {} file(s)", result.changes.len());
        }
    }

    Ok(())
}
