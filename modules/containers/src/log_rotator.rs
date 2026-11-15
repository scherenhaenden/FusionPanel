#![forbid(unsafe_code)]

use std::fs;
use std::io::{self, Error, ErrorKind};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct LogRotator {
    pub max_size_bytes: u64,
    pub max_retained_files: usize,
}

impl LogRotator {
    pub fn new(max_size_bytes: u64, max_retained_files: usize) -> Self {
        Self {
            max_size_bytes,
            max_retained_files,
        }
    }

    pub fn rotate(&self, log_path: &Path) -> io::Result<()> {
        let metadata = fs::metadata(log_path);
        match metadata {
            Ok(m) => {
                if m.len() < self.max_size_bytes {
                    return Ok(()); // No rotation needed
                }
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return Ok(()); // File doesn't exist
            }
            Err(e) => return Err(e),
        }

        self.shift_old_logs(log_path)?;

        let rotated_path = self.rotated_path(log_path, 1);
        fs::rename(log_path, &rotated_path)?;

        // Ensure new empty log file is created with 0o600 permissions
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(log_path)?;

        self.compress(&rotated_path)?;

        Ok(())
    }

    fn shift_old_logs(&self, base_path: &Path) -> io::Result<()> {
        for i in (1..=self.max_retained_files).rev() {
            let current_gz = self.rotated_gz_path(base_path, i);
            let current_uncompressed = self.rotated_path(base_path, i);

            if i == self.max_retained_files {
                if current_gz.exists() {
                    fs::remove_file(&current_gz)?;
                }
                if current_uncompressed.exists() {
                    fs::remove_file(&current_uncompressed)?;
                }
            } else {
                let next_gz = self.rotated_gz_path(base_path, i + 1);
                let next_uncompressed = self.rotated_path(base_path, i + 1);

                if current_gz.exists() {
                    fs::rename(&current_gz, &next_gz)?;
                }
                if current_uncompressed.exists() {
                    fs::rename(&current_uncompressed, &next_uncompressed)?;
                }
            }
        }

        Ok(())
    }

    fn rotated_path(&self, base_path: &Path, index: usize) -> PathBuf {
        let mut path = base_path.as_os_str().to_os_string();
        path.push(format!(".{}", index));
        PathBuf::from(path)
    }

    fn rotated_gz_path(&self, base_path: &Path, index: usize) -> PathBuf {
        let mut path = self.rotated_path(base_path, index).into_os_string();
        path.push(".gz");
        PathBuf::from(path)
    }

    fn compress(&self, path: &Path) -> io::Result<()> {
        let status = Command::new("gzip").arg("-f").arg(path).status()?;

        if !status.success() {
            return Err(Error::other(format!("gzip failed with status: {}", status)));
        }

        Ok(())
    }
}
