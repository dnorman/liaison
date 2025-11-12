use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::resolver::{CycleDetector, Reference, Resolver};
use crate::{html, plaintext};

pub struct FileChange {
    pub path: PathBuf,
    pub new_content: String,
}

/// Process all files and return the changes to be made
pub fn process_files(repo_root: &Path, files: &[PathBuf]) -> Result<Vec<FileChange>> {
    let mut changes = Vec::new();
    let mut resolver = Resolver::new(repo_root.to_path_buf());

    for file in files {
        let content = std::fs::read_to_string(file)
            .with_context(|| format!("Failed to read file: {:?}", file))?;

        let new_content = if is_html_file(file) {
            process_html_file(&content, file, &mut resolver)?
        } else {
            process_plaintext_file(&content, file, &mut resolver)?
        };

        if new_content != content {
            changes.push(FileChange {
                path: file.clone(),
                new_content,
            });
        }
    }

    Ok(changes)
}

fn is_html_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        matches!(ext.to_str(), Some("html") | Some("htm"))
    } else {
        false
    }
}

fn process_html_file(content: &str, _file: &Path, resolver: &mut Resolver) -> Result<String> {
    let blocks = html::find_transclude_blocks(content)?;
    
    if blocks.is_empty() {
        return Ok(content.to_string());
    }

    let mut result = content.to_string();
    
    for block in blocks {
        let reference = Reference::parse(&block.reference)?;
        let mut cycle_detector = CycleDetector::new();
        let resolved_content = resolve_recursive(&reference, resolver, &mut cycle_detector)?;
        
        result = html::replace_inner_html(&result, &block, &resolved_content)?;
    }

    Ok(result)
}

fn process_plaintext_file(content: &str, file: &Path, resolver: &mut Resolver) -> Result<String> {
    let parser = plaintext::PlaintextParser::new(file);
    let blocks = parser.parse(content)?;
    
    let transclude_blocks: Vec<_> = blocks
        .into_iter()
        .filter_map(|b| match b {
            plaintext::Block::Transclude { reference, start_line, end_line } => {
                Some((reference, start_line, end_line))
            }
            _ => None,
        })
        .collect();

    if transclude_blocks.is_empty() {
        return Ok(content.to_string());
    }

    let mut result = content.to_string();
    
    // Process blocks in reverse order to maintain line numbers
    for (reference, start_line, end_line) in transclude_blocks.into_iter().rev() {
        let reference = Reference::parse(&reference)?;
        let mut cycle_detector = CycleDetector::new();
        let resolved_content = resolve_recursive(&reference, resolver, &mut cycle_detector)?;
        
        result = parser.replace_content(&result, start_line, end_line, &resolved_content);
    }

    Ok(result)
}

/// Recursively resolve a reference, expanding any transclude directives in the source
fn resolve_recursive(
    reference: &Reference,
    resolver: &mut Resolver,
    cycle_detector: &mut CycleDetector,
) -> Result<String> {
    cycle_detector.enter(reference)?;
    
    let content = resolver.resolve(reference)?;
    
    // Check if the resolved content has transclude directives
    let expanded = if reference.uri.ends_with(".html") || reference.uri.ends_with(".htm") {
        expand_html_transcludes(&content, resolver, cycle_detector)?
    } else {
        expand_plaintext_transcludes(&content, &reference.uri, resolver, cycle_detector)?
    };
    
    cycle_detector.exit(reference);
    
    Ok(expanded)
}

fn expand_html_transcludes(
    content: &str,
    resolver: &mut Resolver,
    cycle_detector: &mut CycleDetector,
) -> Result<String> {
    let blocks = html::find_transclude_blocks(content)?;
    
    if blocks.is_empty() {
        return Ok(content.to_string());
    }

    let mut result = content.to_string();
    
    for block in blocks {
        let reference = Reference::parse(&block.reference)?;
        let resolved = resolve_recursive(&reference, resolver, cycle_detector)?;
        result = html::replace_inner_html(&result, &block, &resolved)?;
    }

    Ok(result)
}

fn expand_plaintext_transcludes(
    content: &str,
    uri: &str,
    resolver: &mut Resolver,
    cycle_detector: &mut CycleDetector,
) -> Result<String> {
    let path = Path::new(uri);
    let parser = plaintext::PlaintextParser::new(path);
    let blocks = parser.parse(content)?;
    
    let transclude_blocks: Vec<_> = blocks
        .into_iter()
        .filter_map(|b| match b {
            plaintext::Block::Transclude { reference, start_line, end_line } => {
                Some((reference, start_line, end_line))
            }
            _ => None,
        })
        .collect();

    if transclude_blocks.is_empty() {
        return Ok(content.to_string());
    }

    let mut result = content.to_string();
    
    for (reference, start_line, end_line) in transclude_blocks.into_iter().rev() {
        let reference = Reference::parse(&reference)?;
        let resolved = resolve_recursive(&reference, resolver, cycle_detector)?;
        result = parser.replace_content(&result, start_line, end_line, &resolved);
    }

    Ok(result)
}

/// Apply changes to files atomically (all or nothing)
pub fn apply_changes(changes: &[FileChange]) -> Result<()> {
    // First, verify all writes will succeed by doing a dry run
    for change in changes {
        if let Some(parent) = change.path.parent() {
            if !parent.exists() {
                return Err(anyhow::anyhow!("Parent directory does not exist: {:?}", parent));
            }
        }
    }

    // Now apply all changes
    for change in changes {
        std::fs::write(&change.path, &change.new_content)
            .with_context(|| format!("Failed to write file: {:?}", change.path))?;
    }

    Ok(())
}

