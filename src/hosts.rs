//! Host type descriptors for different file formats
//!
//! Each host type knows how to:
//! - Match file extensions it handles
//! - Find transcludes in content
//! - Replace transcludes with resolved content
//! - Apply appropriate indentation

use crate::{html, plaintext};
use anyhow::Result;
use std::path::Path;

/// A transclude block found in content
#[derive(Debug, Clone)]
pub struct TranscludeMatch {
    pub reference: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// Describes how a host type handles transclusion
pub trait HostType: Send + Sync {
    /// Human-readable name for this host type
    fn name(&self) -> &'static str;

    /// Check if this host type applies to the given file path
    fn matches(&self, path: &Path) -> bool;

    /// Find all transcludes in the content
    fn find_transcludes(&self, content: &str, path: &Path) -> Result<Vec<TranscludeMatch>>;

    /// Replace a transclude with resolved content
    fn replace(
        &self,
        content: &str,
        transclude: &TranscludeMatch,
        resolved: &str,
        path: &Path,
    ) -> Result<String>;

    /// Whether this host type applies indentation to transcluded content
    fn applies_indentation(&self) -> bool;
}

// =============================================================================
// HTML Element Host - handles <div transclude="...">
// =============================================================================

pub struct HtmlElementHost;

impl HostType for HtmlElementHost {
    fn name(&self) -> &'static str {
        "HTML Element"
    }

    fn matches(&self, path: &Path) -> bool {
        matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("html") | Some("htm")
        )
    }

    fn find_transcludes(&self, content: &str, _path: &Path) -> Result<Vec<TranscludeMatch>> {
        let blocks = html::find_transclude_blocks(content)?;
        Ok(blocks
            .into_iter()
            .filter(|b| !b.is_attribute_transclude())
            .map(|b| TranscludeMatch {
                reference: b.reference,
                start_line: 0, // Not used for HTML element replacement
                end_line: 0,
            })
            .collect())
    }

    fn replace(
        &self,
        content: &str,
        transclude: &TranscludeMatch,
        resolved: &str,
        _path: &Path,
    ) -> Result<String> {
        let block = html::TranscludeBlock {
            reference: transclude.reference.clone(),
            attribute_name: "transclude".to_string(),
            element_html: String::new(),
            start_pos: 0,
            end_pos: 0,
        };
        let source_is_html = transclude.reference.ends_with(".html")
            || transclude.reference.ends_with(".htm")
            || transclude.reference.ends_with(".md")
            || transclude.reference.ends_with(".markdown");
        html::replace_inner_html(content, &block, resolved, source_is_html)
    }

    fn applies_indentation(&self) -> bool {
        true
    }
}

// =============================================================================
// HTML Comment Host - handles <!-- liaison transclude="..." -->
// =============================================================================

pub struct HtmlCommentHost;

impl HostType for HtmlCommentHost {
    fn name(&self) -> &'static str {
        "HTML Comment"
    }

    fn matches(&self, path: &Path) -> bool {
        matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("html") | Some("htm")
        )
    }

    fn find_transcludes(&self, content: &str, path: &Path) -> Result<Vec<TranscludeMatch>> {
        let parser = plaintext::PlaintextParser::new(path);
        let blocks = parser.parse(content)?;
        Ok(blocks
            .into_iter()
            .filter_map(|b| match b {
                plaintext::Block::Transclude {
                    reference,
                    start_line,
                    end_line,
                } => Some(TranscludeMatch {
                    reference,
                    start_line,
                    end_line,
                }),
                _ => None,
            })
            .collect())
    }

    fn replace(
        &self,
        content: &str,
        transclude: &TranscludeMatch,
        resolved: &str,
        path: &Path,
    ) -> Result<String> {
        let parser = plaintext::PlaintextParser::new(path);
        let lines: Vec<&str> = content.lines().collect();

        // Apply indentation matching the marker line
        let marker_line = lines.get(transclude.start_line).unwrap_or(&"");
        let indent: String = marker_line
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();
        let indented = html::indent_lines(resolved, &indent);

        Ok(parser.replace_content(content, transclude.start_line, transclude.end_line, &indented))
    }

    fn applies_indentation(&self) -> bool {
        true
    }
}

// =============================================================================
// Plaintext Host - handles // liaison transclude, # liaison transclude, etc.
// =============================================================================

pub struct PlaintextHost;

impl HostType for PlaintextHost {
    fn name(&self) -> &'static str {
        "Plaintext"
    }

    fn matches(&self, path: &Path) -> bool {
        // Matches everything except HTML (which has its own hosts)
        !matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("html") | Some("htm")
        )
    }

    fn find_transcludes(&self, content: &str, path: &Path) -> Result<Vec<TranscludeMatch>> {
        let parser = plaintext::PlaintextParser::new(path);
        let blocks = parser.parse(content)?;
        Ok(blocks
            .into_iter()
            .filter_map(|b| match b {
                plaintext::Block::Transclude {
                    reference,
                    start_line,
                    end_line,
                } => Some(TranscludeMatch {
                    reference,
                    start_line,
                    end_line,
                }),
                _ => None,
            })
            .collect())
    }

    fn replace(
        &self,
        content: &str,
        transclude: &TranscludeMatch,
        resolved: &str,
        path: &Path,
    ) -> Result<String> {
        let parser = plaintext::PlaintextParser::new(path);
        Ok(parser.replace_content(
            content,
            transclude.start_line,
            transclude.end_line,
            resolved,
        ))
    }

    fn applies_indentation(&self) -> bool {
        false
    }
}

// =============================================================================
// Host Registry
// =============================================================================

/// Returns all registered host types in order of precedence
pub fn all_hosts() -> Vec<Box<dyn HostType>> {
    vec![
        Box::new(HtmlElementHost),
        Box::new(HtmlCommentHost),
        Box::new(PlaintextHost),
    ]
}

/// Get hosts that match the given file path
pub fn hosts_for_path(path: &Path) -> Vec<Box<dyn HostType>> {
    all_hosts()
        .into_iter()
        .filter(|h| h.matches(path))
        .collect()
}

