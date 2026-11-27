#![forbid(unsafe_code)]

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use thiserror::Error;

/// Errors returned by upgrade preflight validation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PreflightError {
    #[error("Invalid candidate identity: {0}")]
    InvalidCandidateIdentity(String),
    #[error("Invalid {field} '{value}': {reason}")]
    InvalidVersion {
        field: &'static str,
        value: String,
        reason: String,
    },
    #[error("Unsupported upgrade from {current} to {target}: {reason}")]
    UnsupportedSourceVersion {
        current: String,
        target: String,
        reason: String,
    },
    #[error("Duplicate version '{version}' requested without allow_reinstall")]
    DuplicateVersion { version: String },
    #[error(
        "Incompatible platform: expected {expected_os}/{expected_arch}, got {actual_os}/{actual_arch}"
    )]
    IncompatiblePlatform {
        expected_os: String,
        expected_arch: String,
        actual_os: String,
        actual_arch: String,
    },
    #[error("Unsafe path '{}': {reason}", path.display())]
    UnsafePath { path: PathBuf, reason: String },
    #[error(
        "Insufficient disk space at '{}': available {available_bytes} bytes, required {required_bytes} bytes",
        path.display()
    )]
    InsufficientDiskSpace {
        path: PathBuf,
        available_bytes: u64,
        required_bytes: u64,
    },
    #[error("Failed to query available disk space: {0}")]
    DiskSpaceQueryFailed(String),
    #[error("Invalid SHA-256 digest format: {0}")]
    InvalidDigestFormat(String),
    #[error("Artifact SHA-256 digest mismatch: expected {expected}, actual {actual}")]
    ArtifactDigestMismatch { expected: String, actual: String },
    #[error("Artifact signature verification failed: {0}")]
    ArtifactSignatureInvalid(String),
}

/// Parsed semantic version (`MAJOR.MINOR.PATCH[-PRERELEASE]`).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub prerelease: Option<String>,
}

impl SemVer {
    /// Parses a strict semantic version string, rejecting path traversal and invalid characters.
    pub fn parse(input: &str) -> Result<Self, String> {
        if input.is_empty() {
            return Err("version must not be empty".to_string());
        }
        if input.contains('/') || input.contains('\\') || input.contains('\0') {
            return Err("version contains forbidden path characters".to_string());
        }
        if input.chars().any(|c| c.is_whitespace()) {
            return Err("version must not contain whitespace".to_string());
        }
        if input.contains("..") {
            return Err("version must not contain '..'".to_string());
        }

        // Reject build metadata '+' or strip? Keep strict semver characters only.
        let (core, prerelease) = match input.split_once('-') {
            Some((c, pre)) => {
                if pre.is_empty() {
                    return Err("prerelease tag must not be empty".to_string());
                }
                for ident in pre.split('.') {
                    if ident.is_empty() {
                        return Err("prerelease identifier must not be empty".to_string());
                    }
                    if !ident
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-')
                    {
                        return Err(format!(
                            "prerelease identifier '{ident}' contains invalid characters"
                        ));
                    }
                }
                (c, Some(pre.to_string()))
            }
            None => (input, None),
        };

        let parts: Vec<&str> = core.split('.').collect();
        if parts.len() != 3 {
            return Err("version core must have exactly three numeric components".to_string());
        }

        let major = parse_numeric_part(parts[0])?;
        let minor = parse_numeric_part(parts[1])?;
        let patch = parse_numeric_part(parts[2])?;

        Ok(Self {
            major,
            minor,
            patch,
            prerelease,
        })
    }
}

fn parse_numeric_part(part: &str) -> Result<u64, String> {
    if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("invalid numeric version component '{part}'"));
    }
    if part.len() > 1 && part.starts_with('0') {
        return Err(format!(
            "numeric version component '{part}' must not have leading zeros"
        ));
    }
    part.parse::<u64>()
        .map_err(|_| format!("numeric version component '{part}' is out of range"))
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| compare_prerelease(self.prerelease.as_deref(), other.prerelease.as_deref()))
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn compare_prerelease(a: Option<&str>, b: Option<&str>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        // A normal version has higher precedence than a pre-release version.
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(pa), Some(pb)) => {
            let mut ia = pa.split('.');
            let mut ib = pb.split('.');
            loop {
                match (ia.next(), ib.next()) {
                    (None, None) => return Ordering::Equal,
                    (None, Some(_)) => return Ordering::Less,
                    (Some(_), None) => return Ordering::Greater,
                    (Some(sa), Some(sb)) => {
                        let ord = match (sa.parse::<u64>(), sb.parse::<u64>()) {
                            (Ok(na), Ok(nb)) => na.cmp(&nb),
                            (Ok(_), Err(_)) => Ordering::Less,
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => sa.cmp(sb),
                        };
                        if ord != Ordering::Equal {
                            return ord;
                        }
                    }
                }
            }
        }
    }
}

/// Inputs for upgrade preflight validation.
#[derive(Debug, Clone)]
pub struct PreflightRequest {
    pub candidate_id: String,
    pub current_version: String,
    pub target_version: String,
    pub allow_reinstall: bool,
    pub expected_os: String,
    pub expected_arch: String,
    pub actual_os: String,
    pub actual_arch: String,
    pub target_path: PathBuf,
    pub state_paths: Vec<PathBuf>,
    pub required_bytes: u64,
    pub available_bytes: Option<u64>,
}

/// Artifact integrity verification mode for preflight checks.
pub enum ArtifactVerification<'a> {
    Sha256Hex(&'a str),
    Ed25519 {
        signature: &'a [u8],
        public_key: &'a [u8; 32],
    },
}

/// Validates candidate identity against empty values, invalid characters, and path traversal.
pub fn validate_candidate_identity(candidate_id: &str) -> Result<(), PreflightError> {
    if candidate_id.is_empty() {
        return Err(PreflightError::InvalidCandidateIdentity(
            "candidate identity must not be empty".to_string(),
        ));
    }
    if candidate_id.len() > 128 {
        return Err(PreflightError::InvalidCandidateIdentity(
            "candidate identity exceeds 128 characters".to_string(),
        ));
    }
    if candidate_id == "." || candidate_id == ".." || candidate_id.starts_with('.') {
        return Err(PreflightError::InvalidCandidateIdentity(
            "candidate identity must not start with '.' or be '.'/'..'".to_string(),
        ));
    }
    if candidate_id.contains("..")
        || candidate_id.contains('/')
        || candidate_id.contains('\\')
        || candidate_id.contains('\0')
    {
        return Err(PreflightError::InvalidCandidateIdentity(
            "candidate identity contains path traversal or separator characters".to_string(),
        ));
    }
    if !candidate_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(PreflightError::InvalidCandidateIdentity(
            "candidate identity may only contain ASCII alphanumeric characters, '-', '_', or '.'"
                .to_string(),
        ));
    }
    Ok(())
}

/// Parses and validates a semantic version string.
pub fn validate_version_format(
    field: &'static str,
    version: &str,
) -> Result<SemVer, PreflightError> {
    SemVer::parse(version).map_err(|reason| PreflightError::InvalidVersion {
        field,
        value: version.to_string(),
        reason,
    })
}

/// Validates adjacent upgrade rules:
/// - rejects duplicate/same-version upgrades unless `allow_reinstall` is true
/// - rejects downgrades
/// - rejects major version jumps
pub fn validate_version_transition(
    current: &SemVer,
    target: &SemVer,
    allow_reinstall: bool,
    current_raw: &str,
    target_raw: &str,
) -> Result<(), PreflightError> {
    if target == current {
        if allow_reinstall {
            return Ok(());
        }
        return Err(PreflightError::DuplicateVersion {
            version: target_raw.to_string(),
        });
    }

    if target < current {
        return Err(PreflightError::UnsupportedSourceVersion {
            current: current_raw.to_string(),
            target: target_raw.to_string(),
            reason: "downgrade is not allowed".to_string(),
        });
    }

    if target.major != current.major {
        return Err(PreflightError::UnsupportedSourceVersion {
            current: current_raw.to_string(),
            target: target_raw.to_string(),
            reason: "major version jump is not allowed".to_string(),
        });
    }

    Ok(())
}

/// Validates that the candidate OS and architecture match the host platform.
pub fn validate_platform(
    expected_os: &str,
    expected_arch: &str,
    actual_os: &str,
    actual_arch: &str,
) -> Result<(), PreflightError> {
    if expected_os.is_empty()
        || expected_arch.is_empty()
        || actual_os.is_empty()
        || actual_arch.is_empty()
        || !expected_os.eq_ignore_ascii_case(actual_os)
        || !expected_arch.eq_ignore_ascii_case(actual_arch)
    {
        return Err(PreflightError::IncompatiblePlatform {
            expected_os: expected_os.to_string(),
            expected_arch: expected_arch.to_string(),
            actual_os: actual_os.to_string(),
            actual_arch: actual_arch.to_string(),
        });
    }
    Ok(())
}

/// Validates that `target_path` has an existing parent directory, no path traversal,
/// and that neither `target_path` nor any `state_paths` are symlinks.
pub fn validate_paths(target_path: &Path, state_paths: &[PathBuf]) -> Result<(), PreflightError> {
    if target_path.as_os_str().is_empty() {
        return Err(PreflightError::UnsafePath {
            path: target_path.to_path_buf(),
            reason: "target path must not be empty".to_string(),
        });
    }
    if has_parent_traversal(target_path) {
        return Err(PreflightError::UnsafePath {
            path: target_path.to_path_buf(),
            reason: "target path must not contain '..' traversal".to_string(),
        });
    }

    let parent = target_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| PreflightError::UnsafePath {
            path: target_path.to_path_buf(),
            reason: "target path must have a parent directory".to_string(),
        })?;

    if !parent.exists() || !parent.is_dir() {
        return Err(PreflightError::UnsafePath {
            path: parent.to_path_buf(),
            reason: "target parent directory does not exist".to_string(),
        });
    }

    reject_if_symlink(target_path)?;

    for state_path in state_paths {
        if state_path.as_os_str().is_empty() {
            return Err(PreflightError::UnsafePath {
                path: state_path.clone(),
                reason: "state path must not be empty".to_string(),
            });
        }
        if has_parent_traversal(state_path) {
            return Err(PreflightError::UnsafePath {
                path: state_path.clone(),
                reason: "state path must not contain '..' traversal".to_string(),
            });
        }
        reject_if_symlink(state_path)?;
    }

    Ok(())
}

fn has_parent_traversal(path: &Path) -> bool {
    path.components().any(|c| matches!(c, Component::ParentDir))
}

fn reject_if_symlink(path: &Path) -> Result<(), PreflightError> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(PreflightError::UnsafePath {
                    path: path.to_path_buf(),
                    reason: "path must not be a symlink".to_string(),
                });
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(PreflightError::UnsafePath {
            path: path.to_path_buf(),
            reason: format!("failed to inspect path metadata: {e}"),
        }),
    }
}

/// Queries available bytes on the filesystem containing `path` using `df -Pk`.
pub fn query_available_bytes(path: &Path) -> Result<u64, PreflightError> {
    let check_path = if path.exists() {
        path
    } else {
        path.parent()
            .filter(|p| !p.as_os_str().is_empty() && p.exists())
            .ok_or_else(|| {
                PreflightError::DiskSpaceQueryFailed(format!(
                    "no existing path or parent found for '{}'",
                    path.display()
                ))
            })?
    };

    let output = Command::new("df")
        .arg("-Pk")
        .arg(check_path)
        .output()
        .map_err(|e| PreflightError::DiskSpaceQueryFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(PreflightError::DiskSpaceQueryFailed(format!(
            "df -Pk exited with status {}",
            output.status
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let data_line = stdout
        .lines()
        .nth(1)
        .ok_or_else(|| PreflightError::DiskSpaceQueryFailed("unexpected df output".to_string()))?;

    let avail_kb_str = data_line.split_whitespace().nth(3).ok_or_else(|| {
        PreflightError::DiskSpaceQueryFailed("missing available blocks column in df output".to_string())
    })?;

    let avail_kb = avail_kb_str.parse::<u64>().map_err(|e| {
        PreflightError::DiskSpaceQueryFailed(format!("invalid available blocks '{avail_kb_str}': {e}"))
    })?;

    Ok(avail_kb.saturating_mul(1024))
}

/// Validates that at least `required_bytes` are available at `path`.
pub fn validate_disk_space(
    path: &Path,
    required_bytes: u64,
    available_bytes: Option<u64>,
) -> Result<u64, PreflightError> {
    let available = match available_bytes {
        Some(bytes) => bytes,
        None => query_available_bytes(path)?,
    };

    if available < required_bytes {
        return Err(PreflightError::InsufficientDiskSpace {
            path: path.to_path_buf(),
            available_bytes: available,
            required_bytes,
        });
    }

    Ok(available)
}

/// Verifies an artifact payload against an expected 64-character hex SHA-256 digest.
pub fn verify_artifact_sha256(
    artifact_bytes: &[u8],
    expected_hex: &str,
) -> Result<(), PreflightError> {
    if expected_hex.len() != 64 || !expected_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(PreflightError::InvalidDigestFormat(
            "expected a 64-character hexadecimal SHA-256 digest".to_string(),
        ));
    }

    let digest = Sha256::digest(artifact_bytes);
    let mut actual_hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut actual_hex, "{byte:02x}");
    }

    let expected_lower = expected_hex.to_ascii_lowercase();
    if actual_hex != expected_lower {
        return Err(PreflightError::ArtifactDigestMismatch {
            expected: expected_lower,
            actual: actual_hex,
        });
    }

    Ok(())
}

/// Verifies an artifact payload against an Ed25519 signature and 32-byte public key.
pub fn verify_artifact_signature(
    artifact_bytes: &[u8],
    signature_bytes: &[u8],
    public_key_bytes: &[u8; 32],
) -> Result<(), PreflightError> {
    let verifying_key = VerifyingKey::from_bytes(public_key_bytes)
        .map_err(|_| PreflightError::ArtifactSignatureInvalid("invalid public key".to_string()))?;

    let signature = Signature::try_from(signature_bytes)
        .map_err(|_| PreflightError::ArtifactSignatureInvalid("invalid signature format".to_string()))?;

    verifying_key
        .verify(artifact_bytes, &signature)
        .map_err(|_| PreflightError::ArtifactSignatureInvalid("signature mismatch".to_string()))
}

/// Verifies an artifact payload using either SHA-256 digest or Ed25519 signature.
pub fn verify_artifact(
    artifact_bytes: &[u8],
    verification: ArtifactVerification<'_>,
) -> Result<(), PreflightError> {
    match verification {
        ArtifactVerification::Sha256Hex(expected_hex) => {
            verify_artifact_sha256(artifact_bytes, expected_hex)
        }
        ArtifactVerification::Ed25519 {
            signature,
            public_key,
        } => verify_artifact_signature(artifact_bytes, signature, public_key),
    }
}

/// Executes all upgrade preflight checks prior to any download or active-install swap.
pub fn run_preflight(request: &PreflightRequest) -> Result<(), PreflightError> {
    validate_candidate_identity(&request.candidate_id)?;
    let current = validate_version_format("current_version", &request.current_version)?;
    let target = validate_version_format("target_version", &request.target_version)?;
    validate_version_transition(
        &current,
        &target,
        request.allow_reinstall,
        &request.current_version,
        &request.target_version,
    )?;
    validate_platform(
        &request.expected_os,
        &request.expected_arch,
        &request.actual_os,
        &request.actual_arch,
    )?;
    validate_paths(&request.target_path, &request.state_paths)?;
    validate_disk_space(
        &request.target_path,
        request.required_bytes,
        request.available_bytes,
    )?;
    Ok(())
}
