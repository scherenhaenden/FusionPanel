#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct UlimitConfig {
    pub nofile: u64,
    pub nproc: u64,
    pub memlock: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SysctlConfig {
    pub fs_file_max: u64,
    pub net_core_somaxconn: u64,
    pub net_ipv4_tcp_tw_reuse: u8,
    pub vm_swappiness: u8,
}

#[derive(Debug, Clone)]
pub struct PerformanceTuner {
    pub ulimits: UlimitConfig,
    pub sysctls: SysctlConfig,
}

#[derive(Debug)]
pub enum TunerError {
    InvalidUlimit(String),
    InvalidSysctl(String),
    IoError(io::Error),
}

impl From<io::Error> for TunerError {
    fn from(err: io::Error) -> Self {
        TunerError::IoError(err)
    }
}

impl PerformanceTuner {
    pub fn new(ulimits: UlimitConfig, sysctls: SysctlConfig) -> Result<Self, TunerError> {
        let tuner = PerformanceTuner { ulimits, sysctls };
        tuner.validate()?;
        Ok(tuner)
    }

    pub fn validate(&self) -> Result<(), TunerError> {
        // Validate Ulimit
        if self.ulimits.nofile == 0 {
            return Err(TunerError::InvalidUlimit("nofile cannot be zero".into()));
        }
        if self.ulimits.nproc == 0 {
            return Err(TunerError::InvalidUlimit("nproc cannot be zero".into()));
        }

        // Validate Sysctl
        if self.sysctls.fs_file_max == 0 {
            return Err(TunerError::InvalidSysctl("fs.file-max cannot be zero".into()));
        }
        if self.sysctls.net_ipv4_tcp_tw_reuse > 2 {
            return Err(TunerError::InvalidSysctl(
                "net.ipv4.tcp_tw_reuse must be 0, 1, or 2".into(),
            ));
        }
        if self.sysctls.vm_swappiness > 100 {
            return Err(TunerError::InvalidSysctl(
                "vm.swappiness must be between 0 and 100".into(),
            ));
        }

        Ok(())
    }

    pub fn generate_ulimits_conf(&self) -> String {
        format!(
            "# FusionPanel Ulimits Configuration\n\
             * soft nofile {}\n\
             * hard nofile {}\n\
             * soft nproc {}\n\
             * hard nproc {}\n\
             * soft memlock {}\n\
             * hard memlock {}\n",
            self.ulimits.nofile, self.ulimits.nofile,
            self.ulimits.nproc, self.ulimits.nproc,
            self.ulimits.memlock, self.ulimits.memlock,
        )
    }

    pub fn generate_sysctl_conf(&self) -> String {
        format!(
            "# FusionPanel Sysctl Tuning\n\
             fs.file-max = {}\n\
             net.core.somaxconn = {}\n\
             net.ipv4.tcp_tw_reuse = {}\n\
             vm.swappiness = {}\n",
            self.sysctls.fs_file_max,
            self.sysctls.net_core_somaxconn,
            self.sysctls.net_ipv4_tcp_tw_reuse,
            self.sysctls.vm_swappiness,
        )
    }

    pub fn apply_to_paths(&self, limits_path: &Path, sysctl_path: &Path) -> Result<(), TunerError> {
        if let Some(parent) = limits_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = sysctl_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(limits_path, self.generate_ulimits_conf())?;
        fs::write(sysctl_path, self.generate_sysctl_conf())?;
        Ok(())
    }
}
