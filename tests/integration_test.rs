use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn liaison_bin() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // remove test binary name
    path.pop(); // remove 'deps'
    path.push("liaison");
    path
}

/// RAII helper for temporary test files - auto-cleans on drop
struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn new(name: &str, content: &str) -> Self {
        let path = PathBuf::from(format!("tests/fixtures/{}", name));
        fs::write(&path, content).unwrap();
        Self { path }
    }

    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }

    fn read(&self) -> String {
        fs::read_to_string(&self.path).unwrap()
    }

    fn run_liaison(&self) -> std::process::Output {
        Command::new(liaison_bin())
            .arg(self.path())
            .output()
            .unwrap()
    }

    fn run_liaison_with_args(&self, args: &[&str]) -> std::process::Output {
        Command::new(liaison_bin())
            .args(args)
            .arg(self.path())
            .output()
            .unwrap()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

// =============================================================================
// Plaintext transclusion tests
// =============================================================================

#[test]
fn test_plaintext_transclude() {
    let target = "tests/fixtures/target.rs";
    let original = fs::read_to_string(target).unwrap();

    let output = Command::new(liaison_bin()).arg(target).output().unwrap();
    assert!(output.status.success(), "liaison failed: {:?}", output);

    let updated = fs::read_to_string(target).unwrap();
    assert!(updated.contains("fn helper() -> i32"));
    assert!(updated.contains("42"));

    fs::write(target, original).unwrap();
}

#[test]
fn test_reset_plaintext() {
    let temp = TempFile::new(
        "temp_reset.rs",
        r#"// Main file

// liaison transclude="tests/fixtures/source.rs#helper"
// old content here
// liaison end
"#,
    );

    // Populate
    let output = temp.run_liaison();
    assert!(output.status.success());
    assert!(temp.read().contains("fn helper()"));

    // Reset
    let output = temp.run_liaison_with_args(&["--reset"]);
    assert!(output.status.success());

    let reset = temp.read();
    assert!(reset.contains(r#"// liaison transclude="tests/fixtures/source.rs#helper""#));
    assert!(!reset.contains("fn helper()"));
    assert!(reset.contains("// liaison end"));
}

// =============================================================================
// HTML element transclusion tests
// =============================================================================

#[test]
fn test_html_transclude() {
    let target = "tests/fixtures/target.html";
    let original = fs::read_to_string(target).unwrap();

    let output = Command::new(liaison_bin()).arg(target).output().unwrap();
    assert!(output.status.success(), "liaison failed: {:?}", output);

    let updated = fs::read_to_string(target).unwrap();
    assert!(updated.contains("<p>Welcome to the guide</p>"));

    fs::write(target, original).unwrap();
}

#[test]
fn test_reset_html() {
    let temp = TempFile::new(
        "temp_reset.html",
        r#"<!DOCTYPE html>
<html>
<body>
<code class="language-rust" transclude="tests/fixtures/source.rs#helper">old content</code>
</body>
</html>"#,
    );

    // Populate
    let output = temp.run_liaison();
    assert!(output.status.success());
    assert!(temp.read().contains("fn helper()"));

    // Reset
    let output = temp.run_liaison_with_args(&["--reset"]);
    assert!(output.status.success());

    let reset = temp.read();
    assert!(reset.contains(r#"transclude="tests/fixtures/source.rs#helper"></code>"#));
    assert!(!reset.contains("fn helper()"));
}

#[test]
fn test_html_attribute_order_preserved() {
    let temp = TempFile::new(
        "temp_attrs.html",
        r#"<!DOCTYPE html>
<html>
<body>
<code class="language-rust" id="example" data-line="1" transclude="tests/fixtures/source.rs#helper"></code>
</body>
</html>"#,
    );

    let output = temp.run_liaison();
    assert!(output.status.success());

    let updated = temp.read();
    assert!(updated.contains(r#"class="language-rust" id="example" data-line="1" transclude="#));
    assert!(updated.contains("fn helper()"));
}

#[test]
fn test_html_escaping_code_to_html() {
    let temp = TempFile::new(
        "temp_escape.html",
        r#"<!DOCTYPE html>
<html>
<body>
<pre><code transclude="tests/fixtures/code_source.rs#generic-code"></code></pre>
</body>
</html>"#,
    );

    let output = temp.run_liaison();
    assert!(output.status.success());

    let updated = temp.read();
    // Should escape < > from Rust generics
    assert!(
        updated.contains("&lt;T&gt;") || (updated.contains("&lt;") && updated.contains("&gt;"))
    );
}

#[test]
fn test_html_no_escape_html_to_html() {
    let temp = TempFile::new(
        "temp_no_escape.html",
        r#"<!DOCTYPE html>
<html>
<body>
<div transclude="tests/fixtures/source.html#intro"></div>
</body>
</html>"#,
    );

    let output = temp.run_liaison();
    assert!(output.status.success());

    let updated = temp.read();
    // Should NOT escape HTML from HTML source
    assert!(updated.contains("<p>Welcome to the guide</p>"));
    assert!(!updated.contains("&lt;p&gt;"));
}

// =============================================================================
// HTML comment transclusion tests (<!-- liaison transclude -->)
// =============================================================================

#[test]
fn test_html_comment_transclude() {
    let temp = TempFile::new(
        "temp_comment.html",
        r#"<!DOCTYPE html>
<html>
<body>
  <div>
    <!-- liaison transclude="tests/fixtures/tagline.md#company-tagline" -->
    <!-- liaison end -->
  </div>
</body>
</html>"#,
    );

    let output = temp.run_liaison();
    assert!(output.status.success());

    let updated = temp.read();
    assert!(updated.contains("**ACME Corp** - Building the Future"));
}

// =============================================================================
// Recursive transclusion tests
// =============================================================================

#[test]
fn test_recursive_html_to_html_with_comment() {
    // This tests the bug we fixed: HTML transcluding HTML that contains comment transcludes
    let temp = TempFile::new(
        "temp_recursive.html",
        r#"<!DOCTYPE html>
<html>
<body>
    <header transclude="tests/fixtures/header_with_nested.html#banner"></header>
</body>
</html>"#,
    );

    let output = temp.run_liaison();
    assert!(
        output.status.success(),
        "liaison failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let updated = temp.read();
    // Should contain content from header_with_nested.html
    assert!(updated.contains("<h1>Welcome</h1>"));
    // Should ALSO contain the recursively transcluded tagline
    assert!(
        updated.contains("**ACME Corp** - Building the Future"),
        "Recursive transclusion failed - tagline not found. Content:\n{}",
        updated
    );
}

// =============================================================================
// Indentation tests
// =============================================================================

#[test]
fn test_html_element_indentation() {
    let temp = TempFile::new(
        "temp_indent_elem.html",
        r#"<!DOCTYPE html>
<html>
<body>
    <div transclude="tests/fixtures/source.html#intro"></div>
</body>
</html>"#,
    );

    let output = temp.run_liaison();
    assert!(output.status.success());

    let updated = temp.read();
    // Content should be indented to match the <div> element (4 spaces)
    assert!(
        updated.contains("    <p>Welcome to the guide</p>") || updated.contains("\n    <p>"),
        "Element content should be indented. Content:\n{}",
        updated
    );
    // Closing tag should match opening tag indentation
    assert!(updated.contains("    </div>"));
}

#[test]
fn test_html_comment_indentation() {
    let temp = TempFile::new(
        "temp_indent_comment.html",
        r#"<!DOCTYPE html>
<html>
<body>
    <div>
        <!-- liaison transclude="tests/fixtures/tagline.md#company-tagline" -->
        <!-- liaison end -->
    </div>
</body>
</html>"#,
    );

    let output = temp.run_liaison();
    assert!(output.status.success());

    let updated = temp.read();
    // Tagline content should be indented to match the comment marker (8 spaces)
    assert!(
        updated.contains("        **ACME Corp**"),
        "Comment transclude content should match marker indentation. Content:\n{}",
        updated
    );
}

// =============================================================================
// Self-closing tag tests
// =============================================================================

#[test]
fn test_self_closing_tag_transclude() {
    let temp = TempFile::new(
        "temp_selfclose.html",
        r#"<!DOCTYPE html>
<html>
<body>
    <div transclude="tests/fixtures/source.html#intro"/>
</body>
</html>"#,
    );

    let output = temp.run_liaison();
    assert!(output.status.success());

    let updated = temp.read();
    // Self-closing tag should be converted to open/close pair with content
    assert!(
        updated.contains("<div transclude="),
        "Should have opening div"
    );
    assert!(updated.contains("</div>"), "Should have closing div");
    assert!(
        updated.contains("<p>Welcome to the guide</p>"),
        "Should contain transcluded content"
    );
    // Should NOT still be self-closing
    assert!(
        !updated.contains("/>"),
        "Should not have self-closing tag anymore"
    );
}

// =============================================================================
// Error handling tests
// =============================================================================

#[test]
fn test_cycle_detection() {
    let target = "tests/fixtures/cycle_a.rs";
    let output = Command::new(liaison_bin()).arg(target).output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Cycle detected"));
}

#[test]
fn test_check_flag_no_changes() {
    let temp = TempFile::new(
        "temp_check.rs",
        r#"// Main file

// liaison transclude="tests/fixtures/source.rs#helper"
// old content here
// liaison end

fn main() {
    helper();
}
"#,
    );

    // First apply changes
    let output = temp.run_liaison();
    assert!(output.status.success());

    // Now check should return 0 (no changes needed)
    let output = temp.run_liaison_with_args(&["--check"]);
    assert!(
        output.status.success(),
        "check flag should return 0 when no changes needed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
