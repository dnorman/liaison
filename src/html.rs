use anyhow::{anyhow, Result};
use lol_html::{element, html_content::ContentType, rewrite_str, RewriteStrSettings};
use regex::Regex;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct TranscludeBlock {
    pub reference: String,
    #[allow(dead_code)] // Kept for API compatibility
    pub element_html: String,
    #[allow(dead_code)] // Kept for API compatibility
    pub start_pos: usize,
    #[allow(dead_code)] // Kept for API compatibility
    pub end_pos: usize,
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
    
    let re = Regex::new(&opening_pattern)
        .map_err(|e| anyhow!("Failed to create regex: {}", e))?;
    
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

/// Find all elements with transclude attribute
pub fn find_transclude_blocks(html: &str) -> Result<Vec<TranscludeBlock>> {
    let blocks = Rc::new(RefCell::new(Vec::new()));
    let blocks_clone = blocks.clone();

    let settings = RewriteStrSettings {
        element_content_handlers: vec![element!("*[transclude]", move |el| {
            if let Some(reference) = el.get_attribute("transclude") {
                blocks_clone.borrow_mut().push(TranscludeBlock {
                    reference,
                    element_html: String::new(), // Not needed with lol_html
                    start_pos: 0,                // Not needed with lol_html
                    end_pos: 0,                  // Not needed with lol_html
                });
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

/// Replace innerHTML of an element in HTML
/// Uses lol_html streaming rewriter - preserves attribute order and structure
pub fn replace_inner_html(
    html: &str,
    block: &TranscludeBlock,
    new_content: &str,
    source_is_html: bool,
) -> Result<String> {
    // Only escape if source is plaintext (not HTML)
    let content = if source_is_html {
        new_content.to_string()
    } else {
        escape_html(new_content)
    };

    let reference = block.reference.clone();
    let settings = RewriteStrSettings {
        element_content_handlers: vec![element!("*[transclude]", move |el| {
            if el.get_attribute("transclude").as_deref() == Some(reference.as_str()) {
                el.set_inner_content(&content, ContentType::Html);
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
    }

    #[test]
    fn test_replace_preserves_attributes() {
        let html = r#"<code class="rust" id="ex" transclude="test.rs#foo">old</code>"#;
        let block = TranscludeBlock {
            reference: "test.rs#foo".to_string(),
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
}
