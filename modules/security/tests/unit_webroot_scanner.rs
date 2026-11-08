#![forbid(unsafe_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;
use security::{WebrootScanner, Severity};

#[test]
fn test_clean_file_passing() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("index.html");
    fs::write(&file_path, "<html><body>Hello World</body></html>").unwrap();

    let scanner = WebrootScanner::new();
    let report = scanner.scan(dir.path()).unwrap();

    assert_eq!(report.findings.len(), 0);
}

#[test]
fn test_upload_directory_php_detection() {
    let dir = tempdir().unwrap();
    let uploads_dir = dir.path().join("uploads");
    fs::create_dir(&uploads_dir).unwrap();

    let file_path = uploads_dir.join("shell.php");
    fs::write(&file_path, "<?php phpinfo(); ?>").unwrap();

    let scanner = WebrootScanner::new();
    let report = scanner.scan(dir.path()).unwrap();

    assert_eq!(report.findings.len(), 1);
    let finding = &report.findings[0];
    assert_eq!(finding.severity, Severity::Critical);
    assert_eq!(finding.rule_id, "EXECUTABLE_IN_UPLOAD");
    // Ensure the path is correct. Since fs::read_dir returns paths, it should match the canonicalized or similar path.
    // We just check it ends with "shell.php".
    assert!(finding.path.to_string_lossy().ends_with("shell.php"));
}

#[test]
fn test_webshell_pattern_detection() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("hidden_shell.txt");
    fs::write(&file_path, "<?php eval(base64_decode('...')); ?>").unwrap();

    let scanner = WebrootScanner::new();
    let report = scanner.scan(dir.path()).unwrap();

    assert_eq!(report.findings.len(), 1);
    let finding = &report.findings[0];
    assert_eq!(finding.severity, Severity::Critical);
    assert_eq!(finding.rule_id, "WEBSHELL_PATTERN");
}

#[test]
fn test_dangerous_shell_execution_pattern() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("script.php");
    fs::write(&file_path, "<?php system(/home/edward/Development/web-panel/FusionPanel/bin/ls); ?>").unwrap();

    let scanner = WebrootScanner::new();
    let report = scanner.scan(dir.path()).unwrap();

    assert_eq!(report.findings.len(), 1);
    let finding = &report.findings[0];
    assert_eq!(finding.severity, Severity::High);
    assert_eq!(finding.rule_id, "WEBSHELL_PATTERN");
    assert_eq!(finding.message, "Found dangerous shell execution pattern");
}

#[test]
fn test_permission_checks() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("public.txt");
    fs::write(&file_path, "public data").unwrap();

    let mut perms = fs::metadata(&file_path).unwrap().permissions();
    perms.set_mode(0o777); // World writable
    fs::set_permissions(&file_path, perms).unwrap();

    let scanner = WebrootScanner::new();
    let report = scanner.scan(dir.path()).unwrap();

    // We might have a finding for the directory if it's world writable,
    // but we know for sure the file is world writable.
    let writable_findings: Vec<_> = report.findings.iter().filter(|f| f.rule_id == "WORLD_WRITABLE").collect();
    assert!(writable_findings.len() >= 1);

    let finding = writable_findings.iter().find(|f| f.path == file_path).unwrap();
    assert_eq!(finding.severity, Severity::High);
}
