use std::path::Component;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum StorageError {
    #[error("Directory traversal attempted")]
    DirectoryTraversal,
    #[error("Invalid volume name")]
    InvalidVolumeName,
    #[error("Invalid tenant ID")]
    InvalidTenantId,
}

pub fn resolve_volume_path(
    base_dir: &Path,
    tenant_id: &str,
    volume_name: &str,
) -> Result<PathBuf, StorageError> {
    if tenant_id.is_empty()
        || tenant_id.contains('/')
        || tenant_id.contains('\\')
        || tenant_id.contains("..")
    {
        return Err(StorageError::InvalidTenantId);
    }
    if volume_name.is_empty()
        || volume_name.contains('/')
        || volume_name.contains('\\')
        || volume_name.contains("..")
    {
        return Err(StorageError::InvalidVolumeName);
    }

    let mut path = base_dir.to_path_buf();
    path.push(tenant_id);
    path.push(volume_name);

    // Check for traversal
    let mut depth = 0;
    for component in path.components() {
        match component {
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return Err(StorageError::DirectoryTraversal);
                }
            }
            Component::Normal(_) => {
                depth += 1;
            }
            _ => {}
        }
    }

    Ok(path)
}
