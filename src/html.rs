use anyhow::{anyhow, Result};
use scraper::{Html, Selector, ElementRef};

/// Extract content by CSS selector from HTML
pub fn extract_by_selector(html: &str, selector_str: &str) -> Result<String> {
    let document = Html::parse_document(html);
    
    let selector = Selector::parse(selector_str)
        .map_err(|e| anyhow!("Invalid CSS selector '{}': {:?}", selector_str, e))?;

    let element = document
        .select(&selector)
        .next()
        .ok_or_else(|| anyhow!("No element matching '{}' found", selector_str))?;

    Ok(element.inner_html())
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TranscludeBlock {
    pub reference: String,
    pub element_html: String,
    pub start_pos: usize,
    pub end_pos: usize,
}

/// Find all elements with transclude attribute
pub fn find_transclude_blocks(html: &str) -> Result<Vec<TranscludeBlock>> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("[transclude]")
        .map_err(|e| anyhow!("Failed to create selector: {:?}", e))?;

    let mut blocks = Vec::new();

    for element in document.select(&selector) {
        let reference = element
            .value()
            .attr("transclude")
            .ok_or_else(|| anyhow!("Element missing transclude attribute"))?
            .to_string();

        // We need to find the position in the original HTML
        // For now, we'll use a simple approach with the element's HTML
        blocks.push(TranscludeBlock {
            reference,
            element_html: serialize_element(&element),
            start_pos: 0, // Will be computed properly in processor
            end_pos: 0,
        });
    }

    Ok(blocks)
}

/// Serialize an element back to HTML string
fn serialize_element(element: &ElementRef) -> String {
    let tag = element.value().name();
    let mut attrs = String::new();
    
    for (name, value) in element.value().attrs() {
        attrs.push_str(&format!(" {}=\"{}\"", name, value));
    }

    let inner = element.inner_html();
    format!("<{}{}>{}</{}>", tag, attrs, inner, tag)
}

/// Replace innerHTML of an element in HTML
pub fn replace_inner_html(html: &str, block: &TranscludeBlock, new_content: &str) -> Result<String> {
    // Find the element and replace just its innerHTML
    let document = Html::parse_document(html);
    let selector = Selector::parse("[transclude]")
        .map_err(|e| anyhow!("Failed to create selector: {:?}", e))?;

    for element in document.select(&selector) {
        let reference = element.value().attr("transclude").unwrap_or("");
        
        if reference == block.reference {
            // Build the replacement: preserve tag and attributes, replace innerHTML
            let tag = element.value().name();
            let mut attrs = String::new();
            
            for (name, value) in element.value().attrs() {
                attrs.push_str(&format!(" {}=\"{}\"", name, value));
            }

            let new_element = format!("<{}{}>{}</{}>", tag, attrs, new_content, tag);
            
            // Replace in original HTML
            let old_element = serialize_element(&element);
            return Ok(html.replace(&old_element, &new_element));
        }
    }

    Err(anyhow!("Could not find element to replace"))
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
}

