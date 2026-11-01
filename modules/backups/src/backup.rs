use crate::error::BackupError;
use crate::models::{BackupType, DbInfo, FileInfo, Manifest};
use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::Builder;
use uuid::Uuid;

pub struct BackupJob {
    source_dir: PathBuf,
    backup_type: BackupType,
    db_info: Vec<DbInfo>,
    metadata: HashMap<String, String>,
}

impl BackupJob {
    pub fn new<P: AsRef<Path>>(source_dir: P) -> Self {
        Self {
            source_dir: source_dir.as_ref().to_path_buf(),
            backup_type: BackupType::Files,
            db_info: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_db(mut self, db_info: DbInfo) -> Self {
        self.db_info.push(db_info);
        self.backup_type = if self.db_info.is_empty() {
            BackupType::Files
        } else {
            BackupType::Mixed
        };
        self
    }

    pub fn execute<P: AsRef<Path>>(&self, dest_file: P) -> Result<Manifest, BackupError> {
        // First pass: collect files, sizes, and calculate checksums without creating archive
        let mut files_info = Vec::new();
        self.calculate_checksums(&self.source_dir, &self.source_dir, &mut files_info)?;

        let timestamp = chrono::Utc::now().to_rfc3339();
        let manifest = Manifest {
            version: 1,
            backup_id: Uuid::new_v4().to_string(),
            timestamp,
            backup_type: self.backup_type.clone(),
            files: files_info,
            databases: self.db_info.clone(),
            metadata: self.metadata.clone(),
        };
        let manifest_json = serde_json::to_string_pretty(&manifest)?;

        let dest = File::create(&dest_file)?;
        let enc = GzEncoder::new(dest, Compression::default());
        let mut tar = Builder::new(enc);

        // Write manifest first
        let mut header = tar::Header::new_gnu();
        header.set_path("manifest.json").map_err(|e| BackupError::Io(e))?;
        header.set_size(manifest_json.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append(&header, manifest_json.as_bytes())?;

        // Second pass: append files to tar
        self.append_directory(&self.source_dir, &self.source_dir, &mut tar)?;

        tar.into_inner()?.finish()?;

        Ok(manifest)
    }

    fn calculate_checksums(
        &self,
        base_dir: &Path,
        current_dir: &Path,
        files_info: &mut Vec<FileInfo>,
    ) -> Result<(), BackupError> {
        if !current_dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(current_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.calculate_checksums(base_dir, &path, files_info)?;
            } else if path.is_file() {
                let rel_path = path.strip_prefix(base_dir).unwrap();
                let rel_path_str = rel_path.to_string_lossy().into_owned();

                let mut file = File::open(&path)?;
                let mut hasher = Sha256::new();
                let mut buffer = [0; 8192];
                let mut size = 0;
                loop {
                    let count = file.read(&mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    hasher.update(&buffer[..count]);
                    size += count as u64;
                }

                let hash = hex::encode(hasher.finalize());

                files_info.push(FileInfo {
                    path: rel_path_str,
                    size,
                    checksum_sha256: hash,
                });
            }
        }

        Ok(())
    }

    fn append_directory(
        &self,
        base_dir: &Path,
        current_dir: &Path,
        tar: &mut Builder<GzEncoder<File>>,
    ) -> Result<(), BackupError> {
        if !current_dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(current_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.append_directory(base_dir, &path, tar)?;
            } else if path.is_file() {
                let rel_path = path.strip_prefix(base_dir).unwrap();
                let mut file = File::open(&path)?;
                tar.append_file(rel_path, &mut file)?;
            }
        }

        Ok(())
    }
}
