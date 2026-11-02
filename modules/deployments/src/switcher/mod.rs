use std::fs;
use std::io;
use std::os::unix::fs::symlink;
use std::path::Path;

/// Atomically switch a symlink to point to a new target directory.
///
/// This creates a temporary symlink in the same directory as the target link,
/// and then atomically renames the temporary symlink over the target link.
/// This ensures zero downtime during the switch.
pub fn switch_symlink(link_name: &Path, target: &Path) -> io::Result<()> {
    let parent = link_name.parent().unwrap_or(Path::new(""));

    // Create a temporary name for the symlink
    let temp_name = parent.join(format!(".tmp-switch-{}", uuid::Uuid::new_v4()));

    // Create the temporary symlink pointing to the target
    symlink(target, &temp_name)?;

    // Atomically rename the temporary symlink to the final symlink name
    match fs::rename(&temp_name, link_name) {
        Ok(_) => Ok(()),
        Err(e) => {
            // Clean up the temporary symlink if the rename fails
            let _ = fs::remove_file(&temp_name);
            Err(e)
        }
    }
}
