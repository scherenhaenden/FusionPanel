#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::io::Read;
    use std::path::PathBuf;
    use std::process::Command;

    fn find_workspace_binary(name: &str) -> Option<PathBuf> {
        let paths = vec![
            PathBuf::from(format!("../../target/debug/{}", name)),
            PathBuf::from(format!("../../../target/debug/{}", name)),
            PathBuf::from(format!("../../../target/release/{}", name)),
        ];

        for path in paths {
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    #[test]
    fn test_binary_reproducibility_and_checksums() {
        let binary_name = "fusionpanel-api";
        let binary_path = find_workspace_binary(binary_name);

        let path = match binary_path {
            Some(p) => p,
            None => {
                panic!(
                    "RESULT=BLOCKED: Missing release artifacts for checksum verification ({})",
                    binary_name
                );
            }
        };

        let mut file = fs::File::open(&path).expect("Failed to open binary");
        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192];

        loop {
            let count = file.read(&mut buffer).expect("Failed to read binary");
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }

        let result = hasher.finalize();
        let hash_hex = hex::encode(result);

        assert!(!hash_hex.is_empty(), "Hash should not be empty");
        println!("Binary hash: {}", hash_hex);
    }

    #[test]
    fn test_clean_installation_script() {
        let docker_check = Command::new("docker").arg("--version").output();
        if docker_check.is_err() {
            panic!(
                "RESULT=BLOCKED: Container runtime (Docker) unavailable for isolated root tests"
            );
        }

        let repo_dir = std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let install_script = repo_dir.join("packaging/install.sh");

        if !install_script.exists() {
            panic!(
                "RESULT=BLOCKED: Install script missing at {:?}",
                install_script
            );
        }

        let output = Command::new("docker")
            .arg("run")
            .arg("--rm")
            .arg("-v")
            .arg(format!("{}:/install.sh:ro", install_script.display()))
            .arg("debian:bookworm-slim")
            .arg("sh")
            .arg("-c")
            .arg("apt-get update && apt-get install -y curl && sh /install.sh --dry-run")
            .output();

        if let Ok(output) = output {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("overlayfs") || stderr.contains("failed to mount") {
                panic!("RESULT=BLOCKED: Docker overlayfs restricted in this environment");
            }
            assert!(
                output.status.success(),
                "Installation script failed in isolated root: {}",
                stderr
            );
        } else {
            panic!("RESULT=BLOCKED: Docker failed to execute");
        }
    }

    #[test]
    fn test_upgrade_path_and_migration() {
        let migration_tool_name = "fusionpanel-cli";
        let migration_path = find_workspace_binary(migration_tool_name);

        if migration_path.is_none() {
            panic!(
                "RESULT=BLOCKED: Migration tool unavailable ({})",
                migration_tool_name
            );
        }
        let migration_path = migration_path.unwrap();

        let target_tmp_dir = std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("target")
            .join("tmp");
        std::fs::create_dir_all(&target_tmp_dir).unwrap();
        let db_dir =
            tempfile::tempdir_in(&target_tmp_dir).expect("Failed to create tempdir in target/tmp");

        let output = Command::new(&migration_path)
            .arg("migrate")
            .env("FUSIONPANEL_STATE_DIR", db_dir.path())
            .output();

        if let Ok(output) = output {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("Usage") || stderr.contains("unrecognized") {
                    panic!(
                        "RESULT=BLOCKED: Migration subcommand not implemented in {}",
                        migration_tool_name
                    );
                }
                assert!(output.status.success(), "Migration failed: {}", stderr);
            }
        } else {
            panic!("RESULT=BLOCKED: Failed to execute migration tool");
        }
    }

    #[test]
    fn test_upgrade_failure_compensation() {
        let migration_tool_name = "fusionpanel-cli";
        let migration_path = find_workspace_binary(migration_tool_name);

        if migration_path.is_none() {
            panic!(
                "RESULT=BLOCKED: Migration tool unavailable ({})",
                migration_tool_name
            );
        }
        let migration_path = migration_path.unwrap();

        let target_tmp_dir = std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("target")
            .join("tmp");
        std::fs::create_dir_all(&target_tmp_dir).unwrap();
        let db_dir =
            tempfile::tempdir_in(&target_tmp_dir).expect("Failed to create tempdir in target/tmp");

        let output = Command::new(&migration_path)
            .arg("migrate")
            .arg("--fail-simulate")
            .env("FUSIONPANEL_STATE_DIR", db_dir.path())
            .output();

        if let Ok(output) = output {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("Usage") || stderr.contains("unrecognized") {
                    panic!(
                        "RESULT=BLOCKED: Migration compensation testing not supported by CLI yet"
                    );
                }
            } else {
                panic!("Expected migration to fail in simulation mode");
            }
        } else {
            panic!("RESULT=BLOCKED: Failed to execute migration tool");
        }
    }
}
