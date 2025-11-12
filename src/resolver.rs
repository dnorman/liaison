use anyhow::{Context, Result, anyhow};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;

/// Find the git repository root for a given path
pub fn find_repo_root_for_path(path: &PathBuf) -> Result<PathBuf> {
    // Get the directory containing the file (or the directory itself if it's a directory)
    let dir = if path.is_file() {
        path.parent()
            .ok_or_else(|| anyhow!("Could not get parent directory of {:?}", path))?
    } else {
        path.as_path()
    };

    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(dir)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8(output.stdout)?.trim().to_string();
            Ok(PathBuf::from(path))
        }
        _ => {
            // Fallback to the directory itself
            Ok(dir.to_path_buf())
        }
    }
}

/// Find the git repository root, or fallback to CWD
pub fn find_repo_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    find_repo_root_for_path(&cwd)
}

/// Reference to content that needs to be resolved
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Reference {
    pub uri: String,
    pub selector: Option<String>,
}

impl Reference {
    pub fn parse(s: &str) -> Result<Self> {
        if let Some((uri, selector)) = s.split_once('#') {
            Ok(Reference {
                uri: uri.to_string(),
                selector: Some(selector.to_string()),
            })
        } else {
            Ok(Reference {
                uri: s.to_string(),
                selector: None,
            })
        }
    }
}

pub struct Resolver {
    repo_root: PathBuf,
    cache: HashMap<Reference, String>,
}

impl Resolver {
    pub fn new(repo_root: PathBuf) -> Self {
        Self {
            repo_root,
            cache: HashMap::new(),
        }
    }

    /// Resolve a reference to its content
    /// current_file_path is the path to the file containing the reference (for relative resolution)
    /// Returns (content, resolved_path) where resolved_path is the actual file path that was loaded
    pub fn resolve(
        &mut self,
        reference: &Reference,
        current_file_path: Option<&str>,
    ) -> Result<(String, String)> {
        if let Some(cached) = self.cache.get(reference) {
            // For cached content, the resolved path is just the URI
            // (we don't cache the resolved path, but that's okay for now)
            return Ok((cached.clone(), reference.uri.clone()));
        }

        let (content, resolved_path) =
            if reference.uri.starts_with("http://") || reference.uri.starts_with("https://") {
                (self.fetch_http(&reference.uri)?, reference.uri.clone())
            } else {
                self.fetch_local(&reference.uri, current_file_path)?
            };

        let result = if let Some(selector) = &reference.selector {
            self.extract_content(&content, &reference.uri, selector)?
        } else {
            self.extract_default(&content, &reference.uri)?
        };

        self.cache.insert(reference.clone(), result.clone());
        Ok((result, resolved_path))
    }

    fn fetch_http(&self, uri: &str) -> Result<String> {
        let response =
            reqwest::blocking::get(uri).with_context(|| format!("Failed to fetch {}", uri))?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP {} for {}", response.status(), uri));
        }

        response.text().context("Failed to read response body")
    }

    fn fetch_local(&self, path: &str, current_file_path: Option<&str>) -> Result<(String, String)> {
        // Reject paths that try to escape the repo
        if path.contains("..") {
            return Err(anyhow!("Path contains '..' which is not allowed: {}", path));
        }

        // Try file-relative first if we have a current file
        if let Some(current) = current_file_path {
            let current_dir = std::path::Path::new(current).parent();
            if let Some(dir) = current_dir {
                let file_relative = self.repo_root.join(dir).join(path);
                if file_relative.exists() && file_relative.starts_with(&self.repo_root) {
                    let content = std::fs::read_to_string(&file_relative)
                        .with_context(|| format!("Failed to read file: {}", path))?;
                    // Return the repo-relative path
                    let resolved = file_relative
                        .strip_prefix(&self.repo_root)
                        .unwrap_or(&file_relative)
                        .to_string_lossy()
                        .to_string();
                    return Ok((content, resolved));
                }
            }
        }

        // Fall back to repo-relative
        let full_path = self.repo_root.join(path);

        // Verify the resolved path is still within repo
        if !full_path.starts_with(&self.repo_root) {
            return Err(anyhow!("Path escapes repository: {}", path));
        }

        let content = std::fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read file: {}", path))?;
        Ok((content, path.to_string()))
    }

    fn extract_content(&self, content: &str, uri: &str, selector: &str) -> Result<String> {
        if uri.ends_with(".html") || uri.ends_with(".htm") {
            // For HTML, if the selector is just a simple ID (no # prefix), add it
            let css_selector = if !selector.starts_with('#')
                && !selector.starts_with('.')
                && !selector.contains(' ')
                && !selector.contains('>')
                && !selector.contains(',')
            {
                format!("#{}", selector)
            } else {
                selector.to_string()
            };
            crate::html::extract_by_selector(content, &css_selector)
        } else {
            crate::plaintext::extract_by_id(content, uri, selector)
        }
    }

    fn extract_default(&self, content: &str, uri: &str) -> Result<String> {
        if uri.ends_with(".html") || uri.ends_with(".htm") {
            crate::html::extract_by_selector(content, "body")
        } else {
            Ok(content.to_string())
        }
    }
}

/// Detect cycles in transclude references
pub struct CycleDetector {
    visiting: HashSet<Reference>,
    visited: HashSet<Reference>,
}

impl CycleDetector {
    pub fn new() -> Self {
        Self {
            visiting: HashSet::new(),
            visited: HashSet::new(),
        }
    }

    pub fn enter(&mut self, reference: &Reference) -> Result<()> {
        if self.visiting.contains(reference) {
            return Err(anyhow!("Cycle detected: {:?}", reference));
        }
        if self.visited.contains(reference) {
            return Ok(());
        }
        self.visiting.insert(reference.clone());
        Ok(())
    }

    pub fn exit(&mut self, reference: &Reference) {
        self.visiting.remove(reference);
        self.visited.insert(reference.clone());
    }
}
