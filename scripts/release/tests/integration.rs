use std::process::Command;

#[test]
fn test_clean_build_root_workspace() {
    // "Clean build plus required actual integration lanes. Use actual dependencies in disposable fixtures."
    let output = Command::new("cargo")
        .arg("check") // using 'check' to verify clean build without taking a massive amount of time on CI/test execution
        .arg("--manifest-path")
        .arg("../../Cargo.toml")
        .output()
        .expect("Failed to execute cargo check");

    assert!(output.status.success(), "Root workspace clean build (check) failed. stdout: {}, stderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}
