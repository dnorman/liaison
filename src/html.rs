use anyhow::{Result, anyhow};
use lol_html::{RewriteStrSettings, element, html_content::ContentType, rewrite_str};
use regex::Regex;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct TranscludeBlock {
    pub reference: String,
    /// The attribute name that contains the transclude directive
    /// e.g., "transclude" for <div transclude="...">, "src-transclude" for <img src-transclude="...">
    pub attribute_name: String,
    #[allow(dead_code)] // Kept for API compatibility
    pub element_html: String,
    #[allow(dead_code)] // Kept for API compatibility
    pub start_pos: usize,
    #[allow(dead_code)] // Kept for API compatibility
    pub end_pos: usize,
}

impl TranscludeBlock {
    /// Returns true if this is an attribute transclude (e.g., src-transclude)
    /// vs a content transclude (transclude attribute)
    pub fn is_attribute_transclude(&self) -> bool {
        self.attribute_name != "transclude"
    }

    /// Returns the target attribute name (e.g., "src" from "src-transclude")
    pub fn target_attribute(&self) -> Option<String> {
        if self.attribute_name == "transclude" {
            None
        } else {
            self.attribute_name
                .strip_suffix("-transclude")
                .map(|prefix| prefix.to_string())
        }
    }
}

/// Extract content by CSS selector (e.g., #intro)
/// Uses regex for simplicity since lol_html is streaming-focused
pub fn extract_by_selector(html: &str, selector: &str) -> Result<String> {
    // Parse the selector
    let id_value = if let Some(id) = selector.strip_prefix('#') {
        id
    } else {
        return Err(anyhow!("Only ID selectors (#id) are currently supported"));
    };

    // Find the opening tag with this ID
    let opening_pattern = format!(
        r#"(?s)<(\w+)(\s[^>]*\bid\s*=\s*["']{}\s*["'][^>]*)>"#,
        regex::escape(id_value)
    );

    let re = Regex::new(&opening_pattern).map_err(|e| anyhow!("Failed to create regex: {}", e))?;

    if let Some(captures) = re.captures(html) {
        let tag_name = captures.get(1).unwrap().as_str();
        let opening_end = captures.get(0).unwrap().end();

        // Now find the matching closing tag
        let closing_pattern = format!(r#"</{}\s*>"#, regex::escape(tag_name));
        let closing_re = Regex::new(&closing_pattern)
            .map_err(|e| anyhow!("Failed to create closing regex: {}", e))?;

        if let Some(closing_match) = closing_re.find(&html[opening_end..]) {
            let content = &html[opening_end..opening_end + closing_match.start()];
            return Ok(content.to_string());
        }
    }

    Err(anyhow!("No element matching '{}' found", selector))
}

/// Find all elements with transclude or *-transclude attributes
pub fn find_transclude_blocks(html: &str) -> Result<Vec<TranscludeBlock>> {
    let blocks = Rc::new(RefCell::new(Vec::new()));
    let blocks_clone = blocks.clone();

    let settings = RewriteStrSettings {
        element_content_handlers: vec![element!("*", move |el| {
            // Check for regular transclude attribute
            if let Some(reference) = el.get_attribute("transclude") {
                blocks_clone.borrow_mut().push(TranscludeBlock {
                    reference,
                    attribute_name: "transclude".to_string(),
                    element_html: String::new(),
                    start_pos: 0,
                    end_pos: 0,
                });
            }

            // Check for any *-transclude attributes
            for attr_name in el.attributes().iter().map(|a| a.name()) {
                if attr_name != "transclude"
                    && attr_name.ends_with("-transclude")
                    && let Some(reference) = el.get_attribute(&attr_name)
                {
                    blocks_clone.borrow_mut().push(TranscludeBlock {
                        reference,
                        attribute_name: attr_name.clone(),
                        element_html: String::new(),
                        start_pos: 0,
                        end_pos: 0,
                    });
                }
            }

            Ok(())
        })],
        ..RewriteStrSettings::default()
    };

    rewrite_str(html, settings)?;

    Ok(Rc::try_unwrap(blocks).unwrap().into_inner())
}

/// HTML-escape text content for safe inclusion in HTML
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Find the indentation of an element with the given transclude attribute
fn find_element_indentation(html: &str, reference: &str) -> String {
    // Build pattern to find the element with this transclude attribute
    let pattern = format!(r#"transclude\s*=\s*["']{}["']"#, regex::escape(reference));
    let re = Regex::new(&pattern).unwrap();

    if let Some(m) = re.find(html) {
        // Find the start of the line containing this match
        let before = &html[..m.start()];
        if let Some(line_start) = before.rfind('\n') {
            let line = &before[line_start + 1..];
            // Extract leading whitespace
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            return indent;
        }
    }
    String::new()
}

/// Apply indentation to each line of content, matching the target element's indentation
fn indent_content(content: &str, indent: &str) -> String {
    // Empty content = collapsed tags (no newlines)
    if content.is_empty() {
        return String::new();
    }

    if indent.is_empty() {
        return content.to_string();
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();

    for line in &lines {
        if line.trim().is_empty() {
            result.push(String::new());
        } else {
            result.push(format!("{}{}", indent, line));
        }
    }

    // Join with newlines, add newline before opening and before closing tag indent
    format!("\n{}\n{}", result.join("\n"), indent)
}

/// Apply indentation to each line of content (for comment-based transcludes in HTML)
pub fn indent_lines(content: &str, indent: &str) -> String {
    if indent.is_empty() {
        return content.to_string();
    }

    content
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("{}{}", indent, line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Replace innerHTML of an element in HTML
/// Uses lol_html streaming rewriter - preserves attribute order and structure
/// Handles self-closing tags by converting them to open/close pairs
pub fn replace_inner_html(
    html: &str,
    block: &TranscludeBlock,
    new_content: &str,
    source_is_html: bool,
) -> Result<String> {
    // Only escape if source is plaintext (not HTML)
    let escaped = if source_is_html {
        new_content.to_string()
    } else {
        escape_html(new_content)
    };

    // Match the indentation of the target element
    let indent = find_element_indentation(html, &block.reference);
    let content = indent_content(&escaped, &indent);

    let reference = block.reference.clone();
    let content_clone = content.clone();
    let settings = RewriteStrSettings {
        element_content_handlers: vec![element!("*[transclude]", move |el| {
            if el.get_attribute("transclude").as_deref() == Some(reference.as_str()) {
                if el.is_self_closing() {
                    // Self-closing tags need to be replaced entirely
                    // Rebuild the opening tag with all attributes
                    let tag_name = el.tag_name();
                    let mut attrs = Vec::new();
                    for attr in el.attributes() {
                        attrs.push(format!(r#"{}="{}""#, attr.name(), attr.value()));
                    }
                    let attrs_str = if attrs.is_empty() {
                        String::new()
                    } else {
                        format!(" {}", attrs.join(" "))
                    };
                    let replacement = format!(
                        "<{}{}>{}</{}>",
                        tag_name, attrs_str, content_clone, tag_name
                    );
                    el.replace(&replacement, ContentType::Html);
                } else {
                    el.set_inner_content(&content_clone, ContentType::Html);
                }
            }
            Ok(())
        })],
        ..RewriteStrSettings::default()
    };

    Ok(rewrite_str(html, settings)?)
}

/// Replace an attribute value while preserving the *-transclude directive
/// For example: <img src-transclude="logo.png?dataurl"> becomes
/// <img src-transclude="logo.png?dataurl" src="data:image/png;base64,...">
pub fn replace_attribute(html: &str, block: &TranscludeBlock, new_value: &str) -> Result<String> {
    let reference = block.reference.clone();
    let attribute_name = block.attribute_name.clone();
    let target_attr = block
        .target_attribute()
        .ok_or_else(|| anyhow!("Cannot determine target attribute from {}", attribute_name))?;

    let settings = RewriteStrSettings {
        element_content_handlers: vec![element!("*", move |el| {
            if el.get_attribute(&attribute_name).as_deref() == Some(reference.as_str()) {
                el.set_attribute(&target_attr, new_value)?;
            }
            Ok(())
        })],
        ..RewriteStrSettings::default()
    };

    Ok(rewrite_str(html, settings)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_by_id() {
        let html = r#"<section id="intro"><p>Welcome</p></section>"#;
        let result = extract_by_selector(html, "#intro").unwrap();
        assert_eq!(result, "<p>Welcome</p>");
    }

    #[test]
    fn test_find_transclude_blocks() {
        let html = r#"<article transclude="docs/guide.html#intro">old content</article>"#;
        let blocks = find_transclude_blocks(html).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].reference, "docs/guide.html#intro");
        assert_eq!(blocks[0].attribute_name, "transclude");
    }

    #[test]
    fn test_replace_preserves_attributes() {
        let html = r#"<code class="rust" id="ex" transclude="test.rs#foo">old</code>"#;
        let block = TranscludeBlock {
            reference: "test.rs#foo".to_string(),
            attribute_name: "transclude".to_string(),
            element_html: String::new(),
            start_pos: 0,
            end_pos: 0,
        };
        let result = replace_inner_html(html, &block, "new", true).unwrap();

        // Should preserve all attributes in original order
        assert!(result.contains(r#"class="rust""#));
        assert!(result.contains(r#"id="ex""#));
        assert!(result.contains(r#"transclude="test.rs#foo""#));
        assert!(result.contains(">new</code>"));
    }

    #[test]
    fn test_html_escaping() {
        let html = r#"<code transclude="test.rs#foo"></code>"#;
        let block = TranscludeBlock {
            reference: "test.rs#foo".to_string(),
            attribute_name: "transclude".to_string(),
            element_html: String::new(),
            start_pos: 0,
            end_pos: 0,
        };

        // Test escaping for plaintext
        let result = replace_inner_html(html, &block, "<T>", false).unwrap();
        assert!(result.contains("&lt;T&gt;"));

        // Test no escaping for HTML
        let result = replace_inner_html(html, &block, "<p>Hi</p>", true).unwrap();
        assert!(result.contains("<p>Hi</p>"));
        assert!(!result.contains("&lt;p&gt;"));
    }

    #[test]
    fn test_find_attribute_transclude() {
        let html = r#"<img src-transclude="logo.png?dataurl" alt="logo">"#;
        let blocks = find_transclude_blocks(html).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].reference, "logo.png?dataurl");
        assert_eq!(blocks[0].attribute_name, "src-transclude");
        assert!(blocks[0].is_attribute_transclude());
        assert_eq!(blocks[0].target_attribute(), Some("src".to_string()));
    }

    #[test]
    fn test_replace_attribute() {
        let html = r#"<img src-transclude="logo.png?dataurl" alt="logo">"#;
        let block = TranscludeBlock {
            reference: "logo.png?dataurl".to_string(),
            attribute_name: "src-transclude".to_string(),
            element_html: String::new(),
            start_pos: 0,
            end_pos: 0,
        };
        let result = replace_attribute(html, &block, "data:image/png;base64,ABC123").unwrap();

        // Should preserve both attributes
        assert!(result.contains(r#"src-transclude="logo.png?dataurl""#));
        assert!(result.contains(r#"src="data:image/png;base64,ABC123""#));
        assert!(result.contains(r#"alt="logo""#));
    }
}
