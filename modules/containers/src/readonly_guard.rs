#![forbid(unsafe_code)]

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmpfsConfig {
    pub size_mb: u32,
    pub noexec: bool,
    pub nodev: bool,
    pub nosuid: bool,
    pub mode: Option<u32>,
}

impl Default for TmpfsConfig {
    fn default() -> Self {
        Self {
            size_mb: 64,
            noexec: true,
            nodev: true,
            nosuid: true,
            mode: Some(0o1777),
        }
    }
}

impl TmpfsConfig {
    pub fn new(size_mb: u32) -> Self {
        Self {
            size_mb,
            ..Default::default()
        }
    }

    pub fn with_noexec(mut self, noexec: bool) -> Self {
        self.noexec = noexec;
        self
    }

    pub fn with_mode(mut self, mode: u32) -> Self {
        self.mode = Some(mode);
        self
    }

    pub fn to_mount_options(&self) -> String {
        let mut opts = vec![format!("size={}m", self.size_mb)];
        if self.noexec {
            opts.push("noexec".to_string());
        }
        if self.nodev {
            opts.push("nodev".to_string());
        }
        if self.nosuid {
            opts.push("nosuid".to_string());
        }
        if let Some(mode) = self.mode {
            opts.push(format!("mode={:04o}", mode));
        }
        opts.join(",")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOnlyGuard {
    pub read_only_rootfs: bool,
    pub tmpfs_mounts: BTreeMap<String, TmpfsConfig>,
}

impl Default for ReadOnlyGuard {
    fn default() -> Self {
        let mut mounts = BTreeMap::new();
        mounts.insert("/tmp".to_string(), TmpfsConfig::default());

        let mut run_config = TmpfsConfig::default();
        run_config.mode = Some(0o755);
        mounts.insert("/run".to_string(), run_config);

        Self {
            read_only_rootfs: true,
            tmpfs_mounts: mounts,
        }
    }
}

impl ReadOnlyGuard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tmpfs(mut self, path: &str, config: TmpfsConfig) -> Self {
        self.tmpfs_mounts.insert(path.to_string(), config);
        self
    }

    pub fn remove_tmpfs(mut self, path: &str) -> Self {
        self.tmpfs_mounts.remove(path);
        self
    }

    pub fn disable_read_only(mut self) -> Self {
        self.read_only_rootfs = false;
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.read_only_rootfs {
            return Err("Root filesystem must be read-only".to_string());
        }

        if !self.tmpfs_mounts.contains_key("/tmp") {
            return Err("Missing required tmpfs mount for /tmp".to_string());
        }

        if !self.tmpfs_mounts.contains_key("/run") {
            return Err("Missing required tmpfs mount for /run".to_string());
        }

        for (path, config) in &self.tmpfs_mounts {
            if !path.starts_with('/') {
                return Err(format!("Tmpfs path must be absolute: {}", path));
            }
            if !config.nodev {
                return Err(format!("Tmpfs mount for {} must have nodev option enabled", path));
            }
            if !config.nosuid {
                return Err(format!("Tmpfs mount for {} must have nosuid option enabled", path));
            }
        }

        Ok(())
    }

    pub fn generate_docker_args(&self) -> Result<Vec<String>, String> {
        self.validate()?;

        let mut args = vec!["--read-only".to_string()];

        for (path, config) in &self.tmpfs_mounts {
            args.push(format!("--tmpfs={}:{}", path, config.to_mount_options()));
        }

        Ok(args)
    }
}
