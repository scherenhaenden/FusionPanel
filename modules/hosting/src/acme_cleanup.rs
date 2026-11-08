#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::Path;
use std::time::{Duration, SystemTime};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AcmeCleanupSummary {
    pub files_scanned: usize,
    pub files_removed: usize,
    pub bytes_reclaimed: u64,
    pub skipped_unexpired: usize,
}

pub struct AcmeTokenCleaner {
    pub ttl: Duration,
}

impl Default for AcmeTokenCleaner {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(3600), // default 1 hour
        }
    }
}

impl AcmeTokenCleaner {
    pub fn new(ttl: Duration) -> Self {
        Self { ttl }
    }

    pub fn clean_directory<P: AsRef<Path>>(&self, path: P) -> io::Result<AcmeCleanupSummary> {
        let mut summary = AcmeCleanupSummary::default();

        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(summary),
            Err(e) => return Err(e),
        };

        let now = SystemTime::now();

        for entry_res in entries {
            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => continue, // skip unreadable entries
            };

            let file_name = entry.file_name();
            let file_name_str = match file_name.to_str() {
                Some(s) => s,
                None => continue,
            };

            // Validate token filenames: base64url charset [a-zA-Z0-9_-]+
            if !Self::is_valid_token_filename(file_name_str) {
                continue;
            }

            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            if !meta.is_file() {
                continue;
            }

            summary.files_scanned += 1;

            let modified = match meta.modified() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let age = now.duration_since(modified).unwrap_or(Duration::ZERO);

            if age > self.ttl {
                if fs::remove_file(entry.path()).is_ok() {
                    summary.files_removed += 1;
                    summary.bytes_reclaimed += meta.len();
                }
            } else {
                summary.skipped_unexpired += 1;
            }
        }

        Ok(summary)
    }

    pub fn is_valid_token_filename(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        for c in name.chars() {
            if !c.is_ascii_alphanumeric() && c != '_' && c != '-' {
                return false;
            }
        }
        // Additional paranoia checks just in case
        if name.contains("..") || name.contains('/') || name.contains('\\') || name.contains('\0') {
            return false;
        }
        true
    }
}
