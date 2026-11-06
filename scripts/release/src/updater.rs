use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use thiserror::Error;

const RECOVERY_MARKER: &str = ".recovery-marker";
const MARKER_VERSION: &str = "fusionpanel-updater-v1";

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Missing target binary path")]
    MissingTargetPath,
    #[error("an interrupted update must be recovered before another update")]
    RecoveryRequired,
    #[error("the recovery marker belongs to a different target binary")]
    RecoveryTargetMismatch,
    #[error("recovery cannot restore the previous target because its backup is missing")]
    RecoveryIncomplete,
    #[error("the recovery marker is invalid or belongs to an older ambiguous update")]
    InvalidRecoveryMarker,
    #[error(
        "update failed ({operation}); cleanup failed ({recovery}); recovery status needs inspection"
    )]
    RollbackFailed { operation: String, recovery: String },
}

struct TransactionPaths {
    parent: PathBuf,
    target_name: Vec<u8>,
    backup: PathBuf,
    marker: PathBuf,
    legacy_marker: PathBuf,
    staged: PathBuf,
}

pub struct Updater {
    target_path: PathBuf,
}

impl Updater {
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Replaces the target with a staged copy on the target filesystem.
    ///
    /// A per-target durable marker prevents concurrent transactions. The candidate is
    /// staged beside the target, the old inode is backed up without moving the
    /// live path, and a same-filesystem rename atomically replaces the target.
    pub fn update(&self, new_binary_path: &Path) -> Result<(), UpdateError> {
        self.update_with_rename(new_binary_path, &mut |from, to| fs::rename(from, to))
    }

    /// Restores the previous target after an interrupted update.
    ///
    /// Returns false when there is no recovery state. When a backup exists it
    /// takes precedence over the current target, so a crash after replacement
    /// but before transaction cleanup safely rolls back to the previous file.
    pub fn recover(&self) -> Result<bool, UpdateError> {
        let paths = self.transaction_paths()?;

        if !path_exists(&paths.marker)? {
            if path_exists(&paths.legacy_marker)? {
                return self.recover_legacy_marker(&paths);
            }
            let backup_exists = path_exists(&paths.backup)?;
            let staged_exists = path_exists(&paths.staged)?;
            if !backup_exists && !staged_exists {
                return Ok(false);
            }
            return Err(UpdateError::RecoveryRequired);
        }

        let backup_exists = path_exists(&paths.backup)?;
        let target_exists = path_exists(&self.target_path)?;

        if backup_exists {
            fs::rename(&paths.backup, &self.target_path)?;
        } else {
            let marker = fs::read(&paths.marker)?;
            let had_target = parse_marker(&marker, &paths.target_name)?;
            if had_target && !target_exists {
                return Err(UpdateError::RecoveryIncomplete);
            }
            if !had_target && target_exists {
                fs::remove_file(&self.target_path)?;
            }
        }

        remove_if_exists(&paths.staged)?;
        fs::remove_file(&paths.marker)?;
        sync_directory(&paths.parent)?;
        Ok(true)
    }

    fn recover_legacy_marker(&self, paths: &TransactionPaths) -> Result<bool, UpdateError> {
        let mut matching_backup = false;
        for entry in fs::read_dir(&paths.parent)? {
            let entry = entry?;
            let name = entry.file_name();
            if !name.as_bytes().ends_with(b".backup") {
                continue;
            }
            if entry.path() != paths.backup {
                return Err(UpdateError::InvalidRecoveryMarker);
            }
            matching_backup = true;
        }

        if !matching_backup {
            return Err(UpdateError::InvalidRecoveryMarker);
        }

        fs::rename(&paths.backup, &self.target_path)?;
        remove_if_exists(&paths.staged)?;
        fs::remove_file(&paths.legacy_marker)?;
        sync_directory(&paths.parent)?;
        Ok(true)
    }

    pub fn check_recovery(&self) -> Result<bool, io::Error> {
        let paths = self.transaction_paths().map_err(|error| match error {
            UpdateError::Io(error) => error,
            _ => io::Error::new(io::ErrorKind::InvalidInput, error.to_string()),
        })?;
        Ok(path_exists(&paths.marker)?
            || path_exists(&paths.legacy_marker)?
            || path_exists(&paths.backup)?
            || path_exists(&paths.staged)?)
    }

    fn update_with_rename<F>(
        &self,
        new_binary_path: &Path,
        rename: &mut F,
    ) -> Result<(), UpdateError>
    where
        F: FnMut(&Path, &Path) -> io::Result<()>,
    {
        let paths = self.transaction_paths()?;
        let source_metadata = fs::symlink_metadata(new_binary_path)?;
        if !source_metadata.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "replacement source must be a regular file",
            )
            .into());
        }

        if path_exists(&paths.marker)?
            || path_exists(&paths.legacy_marker)?
            || path_exists(&paths.backup)?
            || path_exists(&paths.staged)?
        {
            return Err(UpdateError::RecoveryRequired);
        }

        let had_target = match fs::symlink_metadata(&self.target_path) {
            Ok(metadata) if metadata.file_type().is_dir() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "target binary path is a directory",
                )
                .into());
            }
            Ok(metadata) if metadata.file_type().is_file() => true,
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "existing target must be a regular file",
                )
                .into());
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(error.into()),
        };

        let marker_file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&paths.marker)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(UpdateError::RecoveryRequired);
            }
            Err(error) => return Err(error.into()),
        };
        let marker_data = marker_contents(&paths.target_name, had_target);
        let marker_result = (|| -> io::Result<()> {
            let mut marker_file = marker_file;
            marker_file.write_all(&marker_data)?;
            marker_file.sync_all()?;
            sync_directory(&paths.parent)
        })();
        if let Err(operation) = marker_result {
            return Err(cleanup_error(operation, &paths));
        }

        if let Err(operation) = copy_staged_binary(new_binary_path, &paths.staged) {
            return Err(cleanup_error(operation, &paths));
        }
        if let Err(operation) = sync_directory(&paths.parent) {
            return Err(cleanup_error(operation, &paths));
        }

        if had_target {
            let target_file = match File::open(&self.target_path) {
                Ok(file) => file,
                Err(operation) => return Err(cleanup_error(operation, &paths)),
            };
            if let Err(operation) = target_file.sync_all() {
                return Err(cleanup_error(operation, &paths));
            }
            if let Err(operation) = fs::hard_link(&self.target_path, &paths.backup) {
                return Err(cleanup_error(operation, &paths));
            }
            if let Err(operation) = sync_directory(&paths.parent) {
                return Err(cleanup_error(operation, &paths));
            }
        }

        if let Err(operation) = rename(&paths.staged, &self.target_path) {
            return Err(cleanup_error(operation, &paths));
        }

        // Keep the backup and marker until the installed file is durable.
        sync_directory(&paths.parent)?;
        remove_if_exists(&paths.backup)?;
        sync_directory(&paths.parent)?;
        fs::remove_file(&paths.marker)?;
        sync_directory(&paths.parent)?;
        Ok(())
    }

    fn transaction_paths(&self) -> Result<TransactionPaths, UpdateError> {
        let target_name = self
            .target_path
            .file_name()
            .ok_or(UpdateError::MissingTargetPath)?;
        let parent = self
            .target_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        Ok(TransactionPaths {
            parent: parent.to_path_buf(),
            target_name: target_name.as_bytes().to_vec(),
            backup: parent.join(append_suffix(target_name, ".backup")),
            marker: parent.join(append_prefix_and_suffix(
                target_name,
                ".",
                ".recovery-marker",
            )),
            legacy_marker: parent.join(RECOVERY_MARKER),
            staged: parent.join(append_prefix_and_suffix(target_name, ".", ".update-stage")),
        })
    }
}

fn append_suffix(name: &std::ffi::OsStr, suffix: &str) -> std::ffi::OsString {
    let mut result = name.to_os_string();
    result.push(suffix);
    result
}

fn append_prefix_and_suffix(
    name: &std::ffi::OsStr,
    prefix: &str,
    suffix: &str,
) -> std::ffi::OsString {
    let mut result = std::ffi::OsString::from(prefix);
    result.push(name);
    result.push(suffix);
    result
}

fn copy_staged_binary(source: &Path, staged: &Path) -> io::Result<()> {
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(staged)?;
    io::copy(&mut input, &mut output)?;
    let mut permissions = output.metadata()?.permissions();
    permissions.set_mode(0o755);
    output.set_permissions(permissions)?;
    output.sync_all()
}

fn cleanup_error(operation: io::Error, paths: &TransactionPaths) -> UpdateError {
    if let Err(recovery) = cleanup_transaction(paths) {
        UpdateError::RollbackFailed {
            operation: operation.to_string(),
            recovery: recovery.to_string(),
        }
    } else {
        UpdateError::Io(operation)
    }
}

fn cleanup_transaction(paths: &TransactionPaths) -> io::Result<()> {
    remove_if_exists(&paths.staged)?;
    remove_if_exists(&paths.backup)?;
    remove_if_exists(&paths.marker)?;
    sync_directory(&paths.parent)
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn path_exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

fn marker_contents(target_name: &[u8], had_target: bool) -> Vec<u8> {
    let mut contents = format!("{MARKER_VERSION}\ntarget=").into_bytes();
    for byte in target_name {
        contents.extend_from_slice(format!("{byte:02x}").as_bytes());
    }
    contents.extend_from_slice(
        format!("\nhad_target={}\n", if had_target { "1" } else { "0" }).as_bytes(),
    );
    contents
}

fn parse_marker(contents: &[u8], expected_target: &[u8]) -> Result<bool, UpdateError> {
    let marker = std::str::from_utf8(contents).map_err(|_| UpdateError::InvalidRecoveryMarker)?;
    let mut lines = marker.lines();
    if lines.next() != Some(MARKER_VERSION) {
        return Err(UpdateError::InvalidRecoveryMarker);
    }
    let target_hex = lines
        .next()
        .and_then(|line| line.strip_prefix("target="))
        .ok_or(UpdateError::InvalidRecoveryMarker)?;
    let had_target = match lines.next() {
        Some("had_target=1") => true,
        Some("had_target=0") => false,
        _ => return Err(UpdateError::InvalidRecoveryMarker),
    };
    if lines.next().is_some() {
        return Err(UpdateError::InvalidRecoveryMarker);
    }

    let marker_target = decode_hex(target_hex)?;
    if marker_target != expected_target {
        return Err(UpdateError::RecoveryTargetMismatch);
    }
    Ok(had_target)
}

fn decode_hex(encoded: &str) -> Result<Vec<u8>, UpdateError> {
    if encoded.len() % 2 != 0 {
        return Err(UpdateError::InvalidRecoveryMarker);
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char)
                .to_digit(16)
                .ok_or(UpdateError::InvalidRecoveryMarker)?;
            let low = (pair[1] as char)
                .to_digit(16)
                .ok_or(UpdateError::InvalidRecoveryMarker)?;
            Ok(((high << 4) | low) as u8)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("fusionpanel-updater-{}-{id}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn marker_round_trip_and_target_matching() {
        let name = b"fusionpanel-api";
        assert_eq!(
            parse_marker(&marker_contents(name, true), name).unwrap(),
            true
        );
        assert!(matches!(
            parse_marker(&marker_contents(name, true), b"other"),
            Err(UpdateError::RecoveryTargetMismatch)
        ));
        assert!(matches!(
            parse_marker(b"corrupt", name),
            Err(UpdateError::InvalidRecoveryMarker)
        ));
    }

    #[test]
    fn failed_install_restores_old_binary_and_cleans_transaction() {
        let temp = TempDir::new();
        let target = temp.path().join("fusionpanel-api");
        let source = temp.path().join("candidate");
        fs::write(&target, b"old").unwrap();
        fs::write(&source, b"new").unwrap();
        let updater = Updater::new(&target);

        let result = updater.update_with_rename(&source, &mut |from, to| {
            if from
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".update-stage"))
                && to == target
            {
                assert!(target.exists());
                assert_eq!(fs::read(&target).unwrap(), b"old");
                return Err(io::Error::other("injected candidate rename failure"));
            }
            fs::rename(from, to)
        });

        assert!(matches!(result, Err(UpdateError::Io(_))));
        assert_eq!(fs::read(&target).unwrap(), b"old");
        assert_eq!(fs::read(&source).unwrap(), b"new");
        assert!(!updater.check_recovery().unwrap());
        assert!(!target.with_file_name("fusionpanel-api.backup").exists());
    }

    #[test]
    fn recovery_rolls_back_a_crash_after_candidate_install() {
        let temp = TempDir::new();
        let target = temp.path().join("fusionpanel-api");
        let backup = temp.path().join("fusionpanel-api.backup");
        let updater = Updater::new(&target);
        let paths = updater.transaction_paths().unwrap();
        fs::write(&target, b"new").unwrap();
        fs::write(&backup, b"old").unwrap();
        fs::write(&paths.marker, marker_contents(b"fusionpanel-api", true)).unwrap();

        assert!(updater.check_recovery().unwrap());
        assert!(updater.recover().unwrap());
        assert_eq!(fs::read(&target).unwrap(), b"old");
        assert!(!backup.exists());
        assert!(!updater.check_recovery().unwrap());
    }

    #[test]
    fn recovery_refuses_to_hide_a_missing_previous_binary() {
        let temp = TempDir::new();
        let target = temp.path().join("fusionpanel-api");
        let updater = Updater::new(&target);
        let paths = updater.transaction_paths().unwrap();
        fs::write(&paths.marker, marker_contents(b"fusionpanel-api", true)).unwrap();

        assert!(matches!(
            updater.recover(),
            Err(UpdateError::RecoveryIncomplete)
        ));
        assert!(updater.check_recovery().unwrap());
    }

    #[test]
    fn recovery_removes_a_candidate_when_no_previous_target_existed() {
        let temp = TempDir::new();
        let target = temp.path().join("fusionpanel-api");
        let updater = Updater::new(&target);
        let paths = updater.transaction_paths().unwrap();
        fs::write(&target, b"new").unwrap();
        fs::write(&paths.marker, marker_contents(b"fusionpanel-api", false)).unwrap();

        assert!(updater.recover().unwrap());
        assert!(!target.exists());
        assert!(!updater.check_recovery().unwrap());
    }

    #[test]
    fn legacy_global_marker_without_backup_requires_manual_recovery() {
        let temp = TempDir::new();
        let target = temp.path().join("fusionpanel-api");
        let updater = Updater::new(&target);
        let marker = temp.path().join(RECOVERY_MARKER);
        fs::write(&target, b"previous").unwrap();
        fs::write(&marker, b"update_in_progress").unwrap();

        assert!(matches!(
            updater.recover(),
            Err(UpdateError::InvalidRecoveryMarker)
        ));
        assert_eq!(fs::read(&target).unwrap(), b"previous");
        assert!(marker.exists());
    }

    #[test]
    fn legacy_global_marker_restores_its_matching_backup() {
        let temp = TempDir::new();
        let target = temp.path().join("fusionpanel-api");
        let backup = temp.path().join("fusionpanel-api.backup");
        let updater = Updater::new(&target);
        let marker = temp.path().join(RECOVERY_MARKER);
        fs::write(&target, b"candidate").unwrap();
        fs::write(&backup, b"previous").unwrap();
        fs::write(&marker, b"update_in_progress").unwrap();

        assert!(updater.recover().unwrap());
        assert_eq!(fs::read(&target).unwrap(), b"previous");
        assert!(!backup.exists());
        assert!(!marker.exists());
    }
}
