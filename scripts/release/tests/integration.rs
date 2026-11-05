use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use release_integration::updater::Updater;

/// Custom Drop struct for managing temporary directories securely
/// without external dependencies like `tempfile`, adhering to the memory.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        // Create inside target directory rather than /tmp to avoid cross-device renames (EXDEV)
        // and TOCTOU vulnerabilities, per the memory.
        let target_dir = PathBuf::from("target").join("tmp");
        fs::create_dir_all(&target_dir).unwrap();
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        
        let path = target_dir.join(format!("{}_{}", prefix, timestamp));
        
        // Explicitly set strict POSIX permissions (0o700) using PermissionsExt
        fs::create_dir(&path).unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o700);
        fs::set_permissions(&path, perms).unwrap();
        
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[test]
fn test_clean_build_root_workspace() {
    let output = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg("../../Cargo.toml")
        .output()
        .expect("Failed to execute cargo check");

    assert!(output.status.success(), "Root workspace clean build (check) failed. stdout: {}, stderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}

#[test]
fn test_updater_atomic_swap() {
    let tmp = TempDir::new("test_updater");
    
    let target_bin = tmp.path().join("fusionpanel-api");
    fs::write(&target_bin, b"#!/bin/sh\necho old\n").unwrap();
    
    let mut perms = fs::metadata(&target_bin).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&target_bin, perms).unwrap();

    let new_bin = tmp.path().join("fusionpanel-api-new");
    fs::write(&new_bin, b"#!/bin/sh\necho new\n").unwrap();
    // Start without execute permissions to ensure updater sets them
    let mut perms = fs::metadata(&new_bin).unwrap().permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&new_bin, perms).unwrap();

    let updater = Updater::new(&target_bin);
    
    assert!(!updater.check_recovery().unwrap());
    
    updater.update(&new_bin).unwrap();
    
    assert!(!updater.check_recovery().unwrap()); // marker should be cleaned up
    
    // Target should have new content
    let content = fs::read_to_string(&target_bin).unwrap();
    assert_eq!(content, "#!/bin/sh\necho new\n");
    
    // Target should have executable permissions
    let mode = fs::metadata(&target_bin).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0o111, "Executable bits must be set");
}

#[test]
fn test_updater_execute_mock() {
    let tmp = TempDir::new("test_updater_mock");
    let target_bin = tmp.path().join("fusionpanel-mock");
    
    // Write valid shell script so that process spawning works (per memory)
    fs::write(&target_bin, b"#!/bin/sh\nsleep 1\n").unwrap();
    let mut perms = fs::metadata(&target_bin).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&target_bin, perms).unwrap();
    
    let status = Command::new(&target_bin).status().expect("Mock failed to spawn");
    assert!(status.success());
}
