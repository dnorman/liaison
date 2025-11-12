use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::config::Config;

pub fn discover_files(
    repo_root: &Path,
    config: &Config,
    cli_paths: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let mut files = HashSet::new();

    if !cli_paths.is_empty() {
        // CLI paths override default (empty) glob config
        for path in cli_paths {
            let full_path = if path.is_absolute() {
                path.clone()
            } else {
                repo_root.join(path)
            };

            if full_path.is_file() {
                files.insert(full_path);
            } else if full_path.is_dir() {
                walk_dir(&full_path, &mut files)?;
            } else {
                return Err(anyhow::anyhow!("Path does not exist: {:?}", path));
            }
        }
    } else if !config.glob.include.is_empty() {
        // Use glob config
        for pattern in &config.glob.include {
            let full_pattern = repo_root.join(pattern);
            let pattern_str = full_pattern.to_str()
                .context("Invalid path in glob pattern")?;
            
            for entry in glob::glob(pattern_str)? {
                let path = entry?;
                if path.is_file() {
                    files.insert(path);
                }
            }
        }
    }

    // Apply exclusions
    let mut filtered = Vec::new();
    for file in files {
        if !is_excluded(&file, repo_root, &config.glob.exclude)? {
            filtered.push(file);
        }
    }

    Ok(filtered)
}

fn walk_dir(dir: &Path, files: &mut HashSet<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() {
            files.insert(path);
        } else if path.is_dir() {
            walk_dir(&path, files)?;
        }
    }

    Ok(())
}

fn is_excluded(file: &Path, repo_root: &Path, exclude_patterns: &[String]) -> Result<bool> {
    for pattern in exclude_patterns {
        let full_pattern = repo_root.join(pattern);
        let pattern_str = full_pattern.to_str()
            .context("Invalid path in exclude pattern")?;
        
        if let Ok(entries) = glob::glob(pattern_str) {
            for entry in entries {
                if let Ok(path) = entry {
                    if path == file {
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(false)
}

