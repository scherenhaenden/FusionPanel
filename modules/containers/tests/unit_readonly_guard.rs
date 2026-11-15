#![forbid(unsafe_code)]

use containers::readonly_guard::{ReadOnlyGuard, TmpfsConfig};

#[test]
fn test_default_config() {
    let guard = ReadOnlyGuard::new();
    assert!(guard.read_only_rootfs);
    assert_eq!(guard.tmpfs_mounts.len(), 2);

    let tmp_config = guard.tmpfs_mounts.get("/tmp").unwrap();
    assert_eq!(tmp_config.size_mb, 64);
    assert!(tmp_config.noexec);
    assert!(tmp_config.nodev);
    assert!(tmp_config.nosuid);
    assert_eq!(tmp_config.mode, Some(0o1777));

    let run_config = guard.tmpfs_mounts.get("/run").unwrap();
    assert_eq!(run_config.size_mb, 64);
    assert!(run_config.noexec);
    assert!(run_config.nodev);
    assert!(run_config.nosuid);
    assert_eq!(run_config.mode, Some(0o755));
}

#[test]
fn test_validation_success() {
    let guard = ReadOnlyGuard::new();
    assert!(guard.validate().is_ok());
}

#[test]
fn test_validation_fails_when_rootfs_not_readonly() {
    let guard = ReadOnlyGuard::new().disable_read_only();
    assert_eq!(guard.validate(), Err("Root filesystem must be read-only".to_string()));
}

#[test]
fn test_validation_fails_missing_tmp() {
    let guard = ReadOnlyGuard::new().remove_tmpfs("/tmp");
    assert_eq!(guard.validate(), Err("Missing required tmpfs mount for /tmp".to_string()));
}

#[test]
fn test_validation_fails_missing_run() {
    let guard = ReadOnlyGuard::new().remove_tmpfs("/run");
    assert_eq!(guard.validate(), Err("Missing required tmpfs mount for /run".to_string()));
}

#[test]
fn test_validation_fails_relative_path() {
    let config = TmpfsConfig::default();
    let guard = ReadOnlyGuard::new().with_tmpfs("cache", config);
    assert_eq!(guard.validate(), Err("Tmpfs path must be absolute: cache".to_string()));
}

#[test]
fn test_validation_fails_missing_nodev() {
    let mut config = TmpfsConfig::default();
    config.nodev = false;
    let guard = ReadOnlyGuard::new().with_tmpfs("/custom", config);
    assert_eq!(guard.validate(), Err("Tmpfs mount for /custom must have nodev option enabled".to_string()));
}

#[test]
fn test_validation_fails_missing_nosuid() {
    let mut config = TmpfsConfig::default();
    config.nosuid = false;
    let guard = ReadOnlyGuard::new().with_tmpfs("/custom", config);
    assert_eq!(guard.validate(), Err("Tmpfs mount for /custom must have nosuid option enabled".to_string()));
}

#[test]
fn test_docker_args_generation() {
    let guard = ReadOnlyGuard::new()
        .with_tmpfs("/var/cache", TmpfsConfig::new(128).with_noexec(false).with_mode(0o755));

    let args = guard.generate_docker_args().unwrap();
    assert_eq!(args.len(), 4); // --read-only, and 3 --tmpfs args

    assert_eq!(args[0], "--read-only");

    // Sort order based on BTreeMap keys: /run, /tmp, /var/cache
    assert_eq!(args[1], "--tmpfs=/run:size=64m,noexec,nodev,nosuid,mode=0755");
    assert_eq!(args[2], "--tmpfs=/tmp:size=64m,noexec,nodev,nosuid,mode=1777");
    assert_eq!(args[3], "--tmpfs=/var/cache:size=128m,nodev,nosuid,mode=0755");
}

#[test]
fn test_tmpfs_config_builder() {
    let config = TmpfsConfig::new(256)
        .with_noexec(false)
        .with_mode(0o700);

    assert_eq!(config.size_mb, 256);
    assert!(!config.noexec);
    assert!(config.nodev); // Default
    assert!(config.nosuid); // Default
    assert_eq!(config.mode, Some(0o700));

    let mount_opts = config.to_mount_options();
    assert_eq!(mount_opts, "size=256m,nodev,nosuid,mode=0700");
}
