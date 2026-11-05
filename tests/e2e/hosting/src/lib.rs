#![forbid(unsafe_code)]

pub fn get_temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

pub fn check_dependency(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn block_if_missing(cmd: &str) {
    if !check_dependency(cmd) {
        panic!("BLOCKED: missing {}", cmd);
    }
}
