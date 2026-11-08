#![forbid(unsafe_code)]

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use tempfile::TempDir;
use std::os::unix::fs::MetadataExt;

use backup_destinations::sftp_chroot::{
    validate_chroot_path, SftpChrootBuilder, format_sshd_match_block,
    format_authorized_keys_entry, SftpChrootError,
};

#[test]
fn test_validate_chroot_path_absolute() {
    let relative = Path::new("some/relative/path");
    assert!(matches!(
        validate_chroot_path(relative),
        Err(SftpChrootError::PathNotAbsolute)
    ));

    let absolute = Path::new("/some/absolute/path");
    // This will pass the absolute check. If it doesn't exist it passes symlink check.
    assert!(validate_chroot_path(absolute).is_ok());
}

#[test]
fn test_validate_chroot_path_traversal() {
    let traversal1 = Path::new("/var/www/../html");
    assert!(matches!(
        validate_chroot_path(traversal1),
        Err(SftpChrootError::PathTraversal)
    ));

    let traversal2 = Path::new("/var/www/\0test");
    assert!(matches!(
        validate_chroot_path(traversal2),
        Err(SftpChrootError::PathTraversal)
    ));

    // Valid path that contains ".." in string but is a valid component name
    // e.g. /var/www/..html
    // wait actually we can just check a normal path
    let ok = Path::new("/var/www/..html");
    assert!(validate_chroot_path(ok).is_ok());
}

#[test]
fn test_validate_chroot_path_symlink() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().canonicalize().unwrap();

    // Create a real directory
    let real_dir = temp_path.join("real_dir");
    fs::create_dir(&real_dir).unwrap();

    // Create a symlink to real directory
    let symlink_path = temp_path.join("symlink_dir");
    symlink(&real_dir, &symlink_path).unwrap();

    // Validate that the path containing the symlink is rejected
    assert!(matches!(
        validate_chroot_path(&symlink_path),
        Err(SftpChrootError::ContainsSymlink)
    ));

    // Validate that the real directory is accepted
    assert!(validate_chroot_path(&real_dir).is_ok());
}

#[test]
fn test_format_sshd_match_block() {
    let block = format_sshd_match_block(true, "sftponly", None);
    assert_eq!(
        block,
        "Match Group sftponly\n\tChrootDirectory %h\n\tForceCommand internal-sftp\n\tAllowTcpForwarding no\n\tX11Forwarding no\n"
    );

    let block2 = format_sshd_match_block(false, "backupuser", Some("/chroot/backupuser"));
    assert_eq!(
        block2,
        "Match User backupuser\n\tChrootDirectory /chroot/backupuser\n\tForceCommand internal-sftp\n\tAllowTcpForwarding no\n\tX11Forwarding no\n"
    );
}

#[test]
fn test_format_authorized_keys_entry() {
    let key = "ssh-rsa AAAAB3NzaC1... user@host";
    let formatted = format_authorized_keys_entry(key);
    assert_eq!(
        formatted,
        "command=\"internal-sftp\",no-pty,no-port-forwarding,no-X11-forwarding ssh-rsa AAAAB3NzaC1... user@host"
    );

    // test trimming
    let key_with_newline = "ssh-rsa AAAAB3NzaC1... user@host\n";
    let formatted2 = format_authorized_keys_entry(key_with_newline);
    assert_eq!(
        formatted2,
        "command=\"internal-sftp\",no-pty,no-port-forwarding,no-X11-forwarding ssh-rsa AAAAB3NzaC1... user@host"
    );
}

#[test]
fn test_validate_directory_permissions() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().canonicalize().unwrap();

    let chroot_dir = temp_path.join("chroot");
    let user_dir = chroot_dir.join("user");

    fs::create_dir(&chroot_dir).unwrap();
    fs::create_dir(&user_dir).unwrap();

    let uid = fs::metadata(&chroot_dir).unwrap().uid();

    let builder = SftpChrootBuilder::new(&chroot_dir, &user_dir, uid);

    if uid != 0 {
        let result = builder.validate();
        match result {
            Err(SftpChrootError::InvalidPermissions(msg)) => {
                assert!(msg.contains("owned by root"));
            },
            _ => panic!("Expected InvalidPermissions for non-root owner, got {:?}", result),
        }
    } else {
        // if running as root, this should pass assuming permissions are right
        // We will make sure group and others cannot write
        let mut perms = fs::metadata(&chroot_dir).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
        fs::set_permissions(&chroot_dir, perms).unwrap();

        let mut user_perms = fs::metadata(&user_dir).unwrap().permissions();
        user_perms.set_mode(0o700);
        fs::set_permissions(&user_dir, user_perms).unwrap();

        assert!(builder.validate().is_ok());
    }
}
