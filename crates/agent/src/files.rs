#![forbid(unsafe_code)]

use std::fs::{self, Permissions};
use std::io::{self, Write};
use std::os::unix::fs::{chown, PermissionsExt};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub struct FileManager;

impl FileManager {
    pub fn new() -> Self {
        Self
    }

    pub fn write_file_atomic(
        &self,
        path: &Path,
        content: &[u8],
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> io::Result<()> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let mut temp_file = NamedTempFile::new_in(parent)?;

        temp_file.write_all(content)?;

        if let Some(mode) = mode {
            let perms = Permissions::from_mode(mode);
            fs::set_permissions(temp_file.path(), perms)?;
        }

        if uid.is_some() || gid.is_some() {
            chown(temp_file.path(), uid, gid)?;
        }

        temp_file.persist(path)?;

        Ok(())
    }

    pub fn create_dir_all(
        &self,
        path: &Path,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> io::Result<()> {
        fs::create_dir_all(path)?;

        if let Some(mode) = mode {
            let perms = Permissions::from_mode(mode);
            fs::set_permissions(path, perms)?;
        }

        if uid.is_some() || gid.is_some() {
            chown(path, uid, gid)?;
        }

        Ok(())
    }

    fn normalize_path(path: &Path) -> PathBuf {
        use std::path::Component;
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    normalized.pop();
                }
                Component::CurDir => {}
                _ => normalized.push(component),
            }
        }
        normalized
    }

    pub fn create_symlink(
        &self,
        original: &Path,
        link: &Path,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> io::Result<()> {
        // Check for circular symlink
        if self.is_circular_symlink(original, link)? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Circular symlink detected",
            ));
        }

        std::os::unix::fs::symlink(original, link)?;

        if uid.is_some() || gid.is_some() {
            std::os::unix::fs::lchown(link, uid, gid)?;
        }

        Ok(())
    }

    fn is_circular_symlink(&self, original: &Path, link: &Path) -> io::Result<bool> {
        let link_abs = if link.is_absolute() {
            link.to_path_buf()
        } else {
            std::env::current_dir()?.join(link)
        };
        let link_norm = Self::normalize_path(&link_abs);

        let mut current = original.to_path_buf();
        let mut depth = 0;
        let mut current_link_parent = link_norm.parent().unwrap_or(Path::new("")).to_path_buf();

        while depth < 40 {
            let current_abs = if current.is_absolute() {
                current.clone()
            } else {
                current_link_parent.join(&current)
            };
            let current_norm = Self::normalize_path(&current_abs);

            if current_norm == link_norm {
                return Ok(true);
            }

            match fs::read_link(&current_norm) {
                Ok(next) => {
                    current = next;
                    current_link_parent =
                        current_norm.parent().unwrap_or(Path::new("")).to_path_buf();
                    depth += 1;
                }
                Err(_) => {
                    break;
                }
            }
        }
        Ok(false)
    }
}

impl Default for FileManager {
    fn default() -> Self {
        Self::new()
    }
}
