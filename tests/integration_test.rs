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
    let output = Command::new(liaison_bin())
        .arg(target)
        .output()
        .unwrap();
    
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
    let output = Command::new(liaison_bin())
        .arg(target)
        .output()
        .unwrap();
    
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
    assert!(output.status.success(), "First run failed: {:?}", 
        String::from_utf8_lossy(&output.stderr));
    
    // Now check should return 0 (no changes needed)
    let output = Command::new(liaison_bin())
        .arg("--check")
        .arg(temp_target)
        .output()
        .unwrap();
    
    assert!(output.status.success(), 
        "check flag should return 0 when no changes needed. stderr: {}", 
        String::from_utf8_lossy(&output.stderr));
    
    // Clean up
    fs::remove_file(temp_target).unwrap();
}

#[test]
fn test_cycle_detection() {
    let target = "tests/fixtures/cycle_a.rs";
    
    let output = Command::new(liaison_bin())
        .arg(target)
        .output()
        .unwrap();
    
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Cycle detected"));
}

