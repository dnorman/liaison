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

#[test]
fn test_plaintext_transclude() {
    let target = "tests/fixtures/target.rs";
    let original = fs::read_to_string(target).unwrap();

    // Run liaison
    let output = Command::new(liaison_bin()).arg(target).output().unwrap();

    assert!(output.status.success(), "liaison failed: {:?}", output);

    // Check the file was updated
    let updated = fs::read_to_string(target).unwrap();
    assert!(updated.contains("fn helper() -> i32"));
    assert!(updated.contains("42"));

    // Restore original
    fs::write(target, original).unwrap();
}

#[test]
fn test_html_transclude() {
    let target = "tests/fixtures/target.html";
    let original = fs::read_to_string(target).unwrap();

    // Run liaison
    let output = Command::new(liaison_bin()).arg(target).output().unwrap();

    assert!(output.status.success(), "liaison failed: {:?}", output);

    // Check the file was updated
    let updated = fs::read_to_string(target).unwrap();
    assert!(updated.contains("<p>Welcome to the guide</p>"));

    // Restore original
    fs::write(target, original).unwrap();
}

#[test]
fn test_check_flag_no_changes() {
    // Create a temporary target file
    let temp_target = "tests/fixtures/temp_target.rs";
    let initial_content = r#"// Main file

// liaison transclude="tests/fixtures/source.rs#helper"
// old content here
// liaison end

fn main() {
    helper();
}
"#;
    fs::write(temp_target, initial_content).unwrap();

    // First apply changes
    let output = Command::new(liaison_bin())
        .arg(temp_target)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "First run failed: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Now check should return 0 (no changes needed)
    let output = Command::new(liaison_bin())
        .arg("--check")
        .arg(temp_target)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "check flag should return 0 when no changes needed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Clean up
    fs::remove_file(temp_target).unwrap();
}

#[test]
fn test_cycle_detection() {
    let target = "tests/fixtures/cycle_a.rs";

    let output = Command::new(liaison_bin()).arg(target).output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Cycle detected"));
}

#[test]
fn test_reset_html() {
    let temp_file = "tests/fixtures/temp_reset.html";
    let content = r#"<!DOCTYPE html>
<html>
<body>
<code class="language-rust" transclude="tests/fixtures/source.rs#helper">old content</code>
</body>
</html>"#;

    fs::write(temp_file, content).unwrap();

    // First populate
    let output = Command::new(liaison_bin()).arg(temp_file).output().unwrap();
    assert!(output.status.success());

    let populated = fs::read_to_string(temp_file).unwrap();
    assert!(populated.contains("fn helper()"));

    // Now reset
    let output = Command::new(liaison_bin())
        .arg("--reset")
        .arg(temp_file)
        .output()
        .unwrap();
    assert!(output.status.success());

    let reset = fs::read_to_string(temp_file).unwrap();
    assert!(reset.contains(r#"transclude="tests/fixtures/source.rs#helper"></code>"#));
    assert!(!reset.contains("fn helper()"));

    // Clean up
    fs::remove_file(temp_file).unwrap();
}

#[test]
fn test_reset_plaintext() {
    let temp_file = "tests/fixtures/temp_reset.rs";
    let content = r#"// Main file

// liaison transclude="tests/fixtures/source.rs#helper"
// old content here
// liaison end
"#;

    fs::write(temp_file, content).unwrap();

    // First populate
    let output = Command::new(liaison_bin()).arg(temp_file).output().unwrap();
    assert!(output.status.success());

    let populated = fs::read_to_string(temp_file).unwrap();
    assert!(populated.contains("fn helper()"));

    // Now reset
    let output = Command::new(liaison_bin())
        .arg("--reset")
        .arg(temp_file)
        .output()
        .unwrap();
    assert!(output.status.success());

    let reset = fs::read_to_string(temp_file).unwrap();
    assert!(reset.contains(r#"// liaison transclude="tests/fixtures/source.rs#helper""#));
    assert!(!reset.contains("fn helper()"));
    assert!(reset.contains("// liaison end"));

    // Clean up
    fs::remove_file(temp_file).unwrap();
}

#[test]
fn test_html_attribute_order_preserved() {
    let temp_file = "tests/fixtures/temp_attrs.html";
    let content = r#"<!DOCTYPE html>
<html>
<body>
<code class="language-rust" id="example" data-line="1" transclude="tests/fixtures/source.rs#helper"></code>
</body>
</html>"#;

    fs::write(temp_file, content).unwrap();

    // Populate
    let output = Command::new(liaison_bin()).arg(temp_file).output().unwrap();
    assert!(output.status.success());

    let updated = fs::read_to_string(temp_file).unwrap();
    // Attributes should remain in original order
    assert!(updated.contains(r#"class="language-rust" id="example" data-line="1" transclude="#));
    assert!(updated.contains("fn helper()"));

    // Clean up
    fs::remove_file(temp_file).unwrap();
}

#[test]
fn test_html_escaping_code_to_html() {
    let temp_file = "tests/fixtures/temp_escape.html";
    let content = r#"<!DOCTYPE html>
<html>
<body>
<pre><code transclude="tests/fixtures/code_source.rs#generic-code"></code></pre>
</body>
</html>"#;

    fs::write(temp_file, content).unwrap();

    let output = Command::new(liaison_bin()).arg(temp_file).output().unwrap();
    assert!(output.status.success());

    let updated = fs::read_to_string(temp_file).unwrap();
    // Should escape < > & from Rust code
    assert!(updated.contains("&lt;T&gt;") || updated.contains("&lt;") && updated.contains("&gt;"));

    // Clean up
    fs::remove_file(temp_file).unwrap();
}

#[test]
fn test_html_no_escape_html_to_html() {
    let temp_file = "tests/fixtures/temp_no_escape.html";
    let content = r#"<!DOCTYPE html>
<html>
<body>
<div transclude="tests/fixtures/source.html#intro"></div>
</body>
</html>"#;

    fs::write(temp_file, content).unwrap();

    let output = Command::new(liaison_bin()).arg(temp_file).output().unwrap();
    assert!(output.status.success());

    let updated = fs::read_to_string(temp_file).unwrap();
    // Should NOT escape HTML tags from HTML source
    assert!(updated.contains("<p>Welcome to the guide</p>"));
    assert!(!updated.contains("&lt;p&gt;"));

    // Clean up
    fs::remove_file(temp_file).unwrap();
}
