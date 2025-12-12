use anyhow::{Result, anyhow};
use regex::Regex;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum Block {
    Id {
        id: String,
        start_line: usize,
        end_line: usize,
    },
    Transclude {
        reference: String,
        start_line: usize,
        end_line: usize,
    },
}

pub struct PlaintextParser {
    comment_start: String,
    comment_end: Option<String>,
}

impl PlaintextParser {
    pub fn new(file_path: &Path) -> Self {
        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "rs" | "ts" | "tsx" | "js" | "jsx" => Self {
                comment_start: "//".to_string(),
                comment_end: None,
            },
            "py" | "sh" => Self {
                comment_start: "#".to_string(),
                comment_end: None,
            },
            "md" | "markdown" | "txt" | "html" | "htm" => Self {
                comment_start: "<!--".to_string(),
                comment_end: Some("-->".to_string()),
            },
            _ => Self {
                comment_start: "#".to_string(),
                comment_end: None,
            },
        }
    }

    pub fn parse(&self, content: &str) -> Result<Vec<Block>> {
        let mut blocks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let id_pattern = self.build_pattern("liaison id=(\\S+)");
        let transclude_pattern = self.build_pattern("liaison transclude=\"([^\"]+)\"");
        let end_pattern = self.build_pattern("liaison end");

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();

            if let Some(caps) = id_pattern.captures(line) {
                let id = caps.get(1).unwrap().as_str().to_string();
                let start = i;

                // Find the matching end marker, tracking depth for nested blocks
                let mut end = None;
                let mut depth = 1;
                #[allow(clippy::needless_range_loop)]
                for j in (i + 1)..lines.len() {
                    let j_line = lines[j].trim();

                    // Check if this is a new block start (id or transclude)
                    if id_pattern.is_match(j_line) || transclude_pattern.is_match(j_line) {
                        depth += 1;
                    } else if end_pattern.is_match(j_line) {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(j);
                            break;
                        }
                    }
                }

                if let Some(end_line) = end {
                    blocks.push(Block::Id {
                        id,
                        start_line: start,
                        end_line,
                    });
                    i = end_line + 1;
                } else {
                    return Err(anyhow!("Unclosed 'liaison id' block at line {}", i + 1));
                }
            } else if let Some(caps) = transclude_pattern.captures(line) {
                let reference = caps.get(1).unwrap().as_str().to_string();
                let start = i;

                // Find the matching end marker, tracking depth for nested blocks
                let mut end = None;
                let mut depth = 1;
                #[allow(clippy::needless_range_loop)]
                for j in (i + 1)..lines.len() {
                    let j_line = lines[j].trim();

                    // Check if this is a new block start (id or transclude)
                    if id_pattern.is_match(j_line) || transclude_pattern.is_match(j_line) {
                        depth += 1;
                    } else if end_pattern.is_match(j_line) {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(j);
                            break;
                        }
                    }
                }

                if let Some(end_line) = end {
                    blocks.push(Block::Transclude {
                        reference,
                        start_line: start,
                        end_line,
                    });
                    i = end_line + 1;
                } else {
                    return Err(anyhow!(
                        "Unclosed 'liaison transclude' block at line {}",
                        i + 1
                    ));
                }
            } else {
                i += 1;
            }
        }

        Ok(blocks)
    }

    fn build_pattern(&self, inner: &str) -> Regex {
        let pattern = if let Some(end) = &self.comment_end {
            format!(
                r"^\s*{}\s+{}\s*{}\s*$",
                regex::escape(&self.comment_start),
                inner,
                regex::escape(end)
            )
        } else {
            format!(
                r"^\s*{}\s+{}\s*$",
                regex::escape(&self.comment_start),
                inner
            )
        };
        Regex::new(&pattern).unwrap()
    }

    pub fn replace_content(
        &self,
        content: &str,
        start_line: usize,
        end_line: usize,
        new_content: &str,
    ) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();

        // Keep lines before the block
        for i in 0..=start_line {
            if i < lines.len() {
                result.push(lines[i].to_string());
            }
        }

        // Insert new content
        for line in new_content.lines() {
            result.push(line.to_string());
        }

        // Keep lines after the block
        for line in lines.iter().skip(end_line) {
            result.push(line.to_string());
        }

        let mut output = result.join("\n");

        // Preserve final newline if original had one
        if content.ends_with('\n') && !output.ends_with('\n') {
            output.push('\n');
        }

        output
    }
}

/// Extract content by ID from plaintext source
/// Get the leading whitespace (indentation) of a string
fn get_leading_whitespace(line: &str) -> &str {
    let trimmed_start = line.len() - line.trim_start().len();
    &line[..trimmed_start]
}

/// Remove a prefix from a string if it starts with that prefix, otherwise return the original
fn remove_prefix<'a>(s: &'a str, prefix: &str) -> &'a str {
    s.strip_prefix(prefix).unwrap_or(s)
}

/// Normalize indentation by removing the marker's leading whitespace from all lines
fn normalize_indentation(lines: &[&str], marker_line: &str) -> Vec<String> {
    let marker_indent = get_leading_whitespace(marker_line);

    lines
        .iter()
        .map(|line| {
            // Remove the marker's indentation from each line
            // If the line has less indentation, leave it as-is
            remove_prefix(line, marker_indent).to_string()
        })
        .collect()
}

pub fn extract_by_id(content: &str, uri: &str, id: &str) -> Result<String> {
    // Parse the content to find blocks
    let path = Path::new(uri);
    let parser = PlaintextParser::new(path);
    let blocks = parser.parse(content)?;

    // Find the first matching ID block
    for block in blocks {
        if let Block::Id {
            id: block_id,
            start_line,
            end_line,
        } = block
            && block_id == id
        {
            let lines: Vec<&str> = content.lines().collect();
            let marker_line = lines[start_line];
            let content_lines = &lines[(start_line + 1)..end_line];

            // Normalize indentation based on marker line's indentation
            let normalized = normalize_indentation(content_lines, marker_line);
            return Ok(normalized.join("\n"));
        }
    }

    Err(anyhow!("No block with id '{}' found in {}", id, uri))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_id_block() {
        let content = r#"// liaison id=helper
fn helper() -> i32 { 42 }
// liaison end"#;

        let path = Path::new("test.rs");
        let parser = PlaintextParser::new(path);
        let blocks = parser.parse(content).unwrap();

        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Id { id, .. } => assert_eq!(id, "helper"),
            _ => panic!("Expected Id block"),
        }
    }

    #[test]
    fn test_rust_transclude_block() {
        let content = r#"// liaison transclude="src/lib.rs#helper"
old content
// liaison end"#;

        let path = Path::new("test.rs");
        let parser = PlaintextParser::new(path);
        let blocks = parser.parse(content).unwrap();

        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Transclude { reference, .. } => {
                assert_eq!(reference, "src/lib.rs#helper");
            }
            _ => panic!("Expected Transclude block"),
        }
    }

    #[test]
    fn test_indentation_normalization() {
        let content = r#"fn main() {
    // liaison id=indented-code
    let x = 5;
    if x > 0 {
        println!("positive");
    }
    // liaison end
}"#;

        let result = extract_by_id(content, "test.rs", "indented-code").unwrap();

        // The marker has 4 spaces, so all content should have that removed
        let expected = "let x = 5;\nif x > 0 {\n    println!(\"positive\");\n}";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_end_marker_indentation_ignored() {
        // rustfmt often pushes the end marker to different indentation
        let content = r#"fn example() {
    // liaison id=code
    let x = 5;
    let y = 10;
                      // liaison end
}"#;

        let result = extract_by_id(content, "test.rs", "code").unwrap();

        // End marker's indentation (22 spaces) should not affect extraction
        // Only the start marker's indentation (4 spaces) matters
        let expected = "let x = 5;\nlet y = 10;";
        assert_eq!(result, expected);
    }
}
