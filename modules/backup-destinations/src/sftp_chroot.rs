#![forbid(unsafe_code)]

use std::path::{Component, Path, PathBuf};
use std::fs;
use std::os::unix::fs::MetadataExt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SftpChrootError {
    #[error("Path must be absolute")]
    PathNotAbsolute,
    #[error("Path contains traversal sequences")]
    PathTraversal,
    #[error("Path is a symlink or contains symlinks")]
    ContainsSymlink,
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid permissions: {0}")]
    InvalidPermissions(String),
}

/// Validates that a path is safe for chroot.
/// It must be absolute, contain no traversal components, and no symlinks.
pub fn validate_chroot_path(path: &Path) -> Result<(), SftpChrootError> {
    if !path.is_absolute() {
        return Err(SftpChrootError::PathNotAbsolute);
    }
    let path_str = path.to_string_lossy();
    if path_str.contains('\0') {
        return Err(SftpChrootError::PathTraversal);
    }

    // Check if any component is a ParentDir (..) or if there's a symlink
    let mut current = PathBuf::new();
    current.push("/");

    for component in path.components() {
        if component == Component::ParentDir {
            return Err(SftpChrootError::PathTraversal);
        }
        if component.as_os_str() == "/" {
            continue;
        }
        current.push(component);
        if current.exists() {
            let meta = fs::symlink_metadata(&current)?;
            if meta.file_type().is_symlink() {
                return Err(SftpChrootError::ContainsSymlink);
            }
        }
    }

    Ok(())
}

pub struct SftpChrootBuilder {
    chroot_dir: PathBuf,
    user_dir: PathBuf,
    expected_uid: u32,
}

impl SftpChrootBuilder {
    pub fn new<P1: AsRef<Path>, P2: AsRef<Path>>(
        chroot_dir: P1,
        user_dir: P2,
        expected_uid: u32,
    ) -> Self {
        Self {
            chroot_dir: chroot_dir.as_ref().to_path_buf(),
            user_dir: user_dir.as_ref().to_path_buf(),
            expected_uid,
        }
    }

    /// Validates that the chroot directory (parent) is owned by root with 0755 permissions.
    /// Validates that the user directory (child) is owned by the user and is writable.
    pub fn validate(&self) -> Result<(), SftpChrootError> {
        validate_chroot_path(&self.chroot_dir)?;
        validate_chroot_path(&self.user_dir)?;

        if !self.user_dir.starts_with(&self.chroot_dir) {
            return Err(SftpChrootError::PathTraversal);
        }

        let chroot_meta = fs::metadata(&self.chroot_dir)?;
        if chroot_meta.uid() != 0 {
            return Err(SftpChrootError::InvalidPermissions(
                "Chroot directory must be owned by root".into(),
            ));
        }

        // Check for 0755
        let mode = chroot_meta.mode() & 0o777;
        if mode & 0o022 != 0 {
            return Err(SftpChrootError::InvalidPermissions(
                "Chroot directory must not be group or world writable".into(),
            ));
        }

        if self.user_dir.exists() {
            let user_meta = fs::metadata(&self.user_dir)?;
            if user_meta.uid() != self.expected_uid {
                return Err(SftpChrootError::InvalidPermissions(
                    format!("User directory must be owned by uid {}", self.expected_uid),
                ));
            }

            // Check for user writable
            let user_mode = user_meta.mode() & 0o777;
            if user_mode & 0o200 == 0 {
                return Err(SftpChrootError::InvalidPermissions(
                    "User directory must be writable by the user".into(),
                ));
            }
        }

        Ok(())
    }
}

/// Formats the sshd Match block for a user or group
pub fn format_sshd_match_block(is_group: bool, name: &str, chroot_dir: Option<&str>) -> String {
    let match_type = if is_group { "Group" } else { "User" };
    let chroot_directive = match chroot_dir {
        Some(dir) => dir,
        None => "%h", // default to user's home directory
    };

    format!(
        "Match {} {}\n\
        \tChrootDirectory {}\n\
        \tForceCommand internal-sftp\n\
        \tAllowTcpForwarding no\n\
        \tX11Forwarding no\n",
        match_type, name, chroot_directive
    )
}

/// Formats the authorized_keys entry with restrictions
pub fn format_authorized_keys_entry(public_key: &str) -> String {
    format!(
        "command=\"internal-sftp\",no-pty,no-port-forwarding,no-X11-forwarding {}",
        public_key.trim()
    )
}
