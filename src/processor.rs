use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::resolver::{CycleDetector, Reference, Resolver};
use crate::{html, plaintext};

pub struct FileChange {
    pub path: PathBuf,
    pub new_content: String,
}

pub struct ProcessingResult {
    pub changes: Vec<FileChange>,
    pub dependencies: DependencyTree,
    pub errors: Vec<String>,
}

#[derive(Debug, Default)]
pub struct DependencyTree {
    // Maps a file path to the list of files it depends on
    pub deps: HashMap<String, Vec<String>>,
}

impl DependencyTree {
    pub fn add_dependency(&mut self, file: String, depends_on: String) {
        self.deps.entry(file).or_default().push(depends_on);
    }

    pub fn print_tree(&self, root_files: &[PathBuf], repo_root: &std::path::Path) {
        eprintln!("Processing files:");
        for (i, root) in root_files.iter().enumerate() {
            // Use repo-relative path for consistency
            let root_str = root
                .strip_prefix(repo_root)
                .unwrap_or(root)
                .display()
                .to_string();
            let is_last = i == root_files.len() - 1;
            self.print_node(&root_str, "", is_last);
        }
    }

    fn print_node(&self, file: &str, prefix: &str, is_last: bool) {
        let connector = if is_last { "└── " } else { "├── " };
        eprintln!("{}{}{}", prefix, connector, file);

        if let Some(deps) = self.deps.get(file) {
            let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

            for (i, dep) in deps.iter().enumerate() {
                let is_last_dep = i == deps.len() - 1;
                self.print_node(dep, &new_prefix, is_last_dep);
            }
        }
    }
}

/// Reset all transclude blocks to empty content
pub fn reset_files(files: &[PathBuf]) -> Result<Vec<FileChange>> {
    let mut changes = Vec::new();

    for file in files {
        let content = std::fs::read_to_string(file)
            .with_context(|| format!("Failed to read file: {:?}", file))?;

        let new_content = if is_html_file(file) {
            reset_html_file(&content)?
        } else {
            reset_plaintext_file(&content, file)?
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

/// Process all files and return the changes to be made
pub fn process_files(
    repo_root: &Path,
    files: &[PathBuf],
    ignore_errors: bool,
) -> Result<ProcessingResult> {
    let mut changes = Vec::new();
    let mut resolver = Resolver::new(repo_root.to_path_buf());
    let mut dependencies = DependencyTree::default();
    let mut errors = Vec::new();

    for file in files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                let error_msg = format!("Failed to read file {:?}: {}", file, e);
                if ignore_errors {
                    errors.push(error_msg);
                    continue;
                } else {
                    return Err(anyhow::anyhow!(error_msg));
                }
            }
        };

        // Use repo-relative path for dependency tracking
        let file_str = file
            .strip_prefix(repo_root)
            .unwrap_or(file)
            .display()
            .to_string();

        let new_content = if is_html_file(file) {
            match process_html_file(
                &content,
                file,
                &mut resolver,
                &mut dependencies,
                &file_str,
                ignore_errors,
            ) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(format!("Error processing {:?}: {}", file, e));
                    if ignore_errors {
                        content.clone() // Keep original content on error
                    } else {
                        return Err(e);
                    }
                }
            }
        } else {
            match process_plaintext_file(
                &content,
                file,
                &mut resolver,
                &mut dependencies,
                &file_str,
                ignore_errors,
            ) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(format!("Error processing {:?}: {}", file, e));
                    if ignore_errors {
                        content.clone() // Keep original content on error
                    } else {
                        return Err(e);
                    }
                }
            }
        };

        if new_content != content {
            changes.push(FileChange {
                path: file.clone(),
                new_content,
            });
        }
    }

    Ok(ProcessingResult {
        changes,
        dependencies,
        errors,
    })
}

fn is_html_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        matches!(
            ext.to_str(),
            Some("html") | Some("htm") | Some("md") | Some("markdown")
        )
    } else {
        false
    }
}

fn reset_html_file(content: &str) -> Result<String> {
    let blocks = html::find_transclude_blocks(content)?;

    if blocks.is_empty() {
        return Ok(content.to_string());
    }

    let mut result = content.to_string();

    for block in blocks.iter() {
        if block.is_attribute_transclude() {
            // For attribute transcludes, we'd need to remove the target attribute
            // For now, skip attribute transcludes during reset since they don't have
            // "old content" to clear - they just don't have the attribute set yet
            continue;
        } else {
            result = html::replace_inner_html(&result, block, "", false)?;
        }
    }

    Ok(result)
}

fn reset_plaintext_file(content: &str, file: &Path) -> Result<String> {
    let parser = plaintext::PlaintextParser::new(file);
    let blocks = parser.parse(content)?;

    let transclude_blocks: Vec<_> = blocks
        .into_iter()
        .filter_map(|b| match b {
            plaintext::Block::Transclude {
                reference: _,
                start_line,
                end_line,
            } => Some((start_line, end_line)),
            _ => None,
        })
        .collect();

    if transclude_blocks.is_empty() {
        return Ok(content.to_string());
    }

    let mut result = content.to_string();

    // Process blocks in reverse order to maintain line numbers
    for (start_line, end_line) in transclude_blocks.into_iter().rev() {
        result = parser.replace_content(&result, start_line, end_line, "");
    }

    Ok(result)
}

fn process_html_file(
    content: &str,
    _file: &Path,
    resolver: &mut Resolver,
    dependencies: &mut DependencyTree,
    current_file: &str,
    ignore_errors: bool,
) -> Result<String> {
    let blocks = html::find_transclude_blocks(content)?;

    if blocks.is_empty() {
        return Ok(content.to_string());
    }

    let mut result = content.to_string();

    for block in blocks {
        let reference = match Reference::parse(&block.reference) {
            Ok(r) => r,
            Err(e) => {
                if ignore_errors {
                    continue; // Skip this block
                } else {
                    return Err(e);
                }
            }
        };

        let mut cycle_detector = CycleDetector::new();
        let resolved_content = match resolve_recursive(
            &reference,
            resolver,
            &mut cycle_detector,
            dependencies,
            current_file,
        ) {
            Ok(c) => c,
            Err(e) => {
                if ignore_errors {
                    continue; // Skip this block
                } else {
                    return Err(e);
                }
            }
        };

        if block.is_attribute_transclude() {
            // For attribute transcludes (e.g., src-transclude), set the target attribute value
            result = html::replace_attribute(&result, &block, &resolved_content)?;
        } else {
            // For content transcludes, replace innerHTML
            // Don't escape if source is HTML or Markdown (which can contain HTML)
            let source_is_html_like = reference.uri.ends_with(".html")
                || reference.uri.ends_with(".htm")
                || reference.uri.ends_with(".md")
                || reference.uri.ends_with(".markdown");
            result =
                html::replace_inner_html(&result, &block, &resolved_content, source_is_html_like)?;
        }
    }

    Ok(result)
}

fn process_plaintext_file(
    content: &str,
    file: &Path,
    resolver: &mut Resolver,
    dependencies: &mut DependencyTree,
    current_file: &str,
    ignore_errors: bool,
) -> Result<String> {
    let parser = plaintext::PlaintextParser::new(file);
    let blocks = parser.parse(content)?;

    let transclude_blocks: Vec<_> = blocks
        .into_iter()
        .filter_map(|b| match b {
            plaintext::Block::Transclude {
                reference,
                start_line,
                end_line,
            } => Some((reference, start_line, end_line)),
            _ => None,
        })
        .collect();

    if transclude_blocks.is_empty() {
        return Ok(content.to_string());
    }

    let mut result = content.to_string();

    // Process blocks in reverse order to maintain line numbers
    for (reference, start_line, end_line) in transclude_blocks.into_iter().rev() {
        let reference = match Reference::parse(&reference) {
            Ok(r) => r,
            Err(e) => {
                if ignore_errors {
                    continue; // Skip this block
                } else {
                    return Err(e);
                }
            }
        };

        let mut cycle_detector = CycleDetector::new();
        let resolved_content = match resolve_recursive(
            &reference,
            resolver,
            &mut cycle_detector,
            dependencies,
            current_file,
        ) {
            Ok(c) => c,
            Err(e) => {
                if ignore_errors {
                    continue; // Skip this block
                } else {
                    return Err(e);
                }
            }
        };

        result = parser.replace_content(&result, start_line, end_line, &resolved_content);
    }

    Ok(result)
}

/// Recursively resolve a reference, expanding any transclude directives in the source
fn resolve_recursive(
    reference: &Reference,
    resolver: &mut Resolver,
    cycle_detector: &mut CycleDetector,
    dependencies: &mut DependencyTree,
    current_file: &str,
) -> Result<String> {
    cycle_detector.enter(reference)?;

    // Track the dependency
    dependencies.add_dependency(current_file.to_string(), reference.uri.clone());

    let (content, resolved_path) = resolver.resolve(reference, Some(current_file))?;

    // Check if the resolved content has transclude directives
    let expanded = if reference.uri.ends_with(".html") || reference.uri.ends_with(".htm") {
        expand_html_transcludes(
            &content,
            resolver,
            cycle_detector,
            dependencies,
            &resolved_path,
        )?
    } else {
        expand_plaintext_transcludes(
            &content,
            &resolved_path,
            resolver,
            cycle_detector,
            dependencies,
        )?
    };

    cycle_detector.exit(reference);

    Ok(expanded)
}

fn expand_html_transcludes(
    content: &str,
    resolver: &mut Resolver,
    cycle_detector: &mut CycleDetector,
    dependencies: &mut DependencyTree,
    current_file: &str,
) -> Result<String> {
    let blocks = html::find_transclude_blocks(content)?;

    if blocks.is_empty() {
        return Ok(content.to_string());
    }

    let mut result = content.to_string();

    for block in blocks {
        let reference = Reference::parse(&block.reference)?;
        // Don't escape if source is HTML or Markdown (which can contain HTML)
        let source_is_html_like = reference.uri.ends_with(".html")
            || reference.uri.ends_with(".htm")
            || reference.uri.ends_with(".md")
            || reference.uri.ends_with(".markdown");
        let resolved = resolve_recursive(
            &reference,
            resolver,
            cycle_detector,
            dependencies,
            current_file,
        )?;
        result = html::replace_inner_html(&result, &block, &resolved, source_is_html_like)?;
    }

    Ok(result)
}

fn expand_plaintext_transcludes(
    content: &str,
    uri: &str,
    resolver: &mut Resolver,
    cycle_detector: &mut CycleDetector,
    dependencies: &mut DependencyTree,
) -> Result<String> {
    let path = Path::new(uri);
    let parser = plaintext::PlaintextParser::new(path);
    let blocks = parser.parse(content)?;

    let transclude_blocks: Vec<_> = blocks
        .into_iter()
        .filter_map(|b| match b {
            plaintext::Block::Transclude {
                reference,
                start_line,
                end_line,
            } => Some((reference, start_line, end_line)),
            _ => None,
        })
        .collect();

    if transclude_blocks.is_empty() {
        return Ok(content.to_string());
    }

    let mut result = content.to_string();

    for (reference, start_line, end_line) in transclude_blocks.into_iter().rev() {
        let reference = Reference::parse(&reference)?;
        let resolved = resolve_recursive(&reference, resolver, cycle_detector, dependencies, uri)?;
        result = parser.replace_content(&result, start_line, end_line, &resolved);
    }

    Ok(result)
}

/// Apply changes to files atomically (all or nothing)
pub fn apply_changes(changes: &[FileChange]) -> Result<()> {
    // First, verify all writes will succeed by doing a dry run
    for change in changes {
        if let Some(parent) = change.path.parent() {
            // Empty parent means current directory, which exists
            if !parent.as_os_str().is_empty() && !parent.exists() {
                return Err(anyhow::anyhow!(
                    "Parent directory does not exist: {:?}",
                    parent
                ));
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
