#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write;

/// Represents an AppArmor profile generator for FusionPanel applications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppArmorProfile {
    pub name: String,
    pub executable_path: String,
    pub paths: BTreeMap<String, String>,
    pub network_families: Vec<String>,
    pub disable_core_dumps: bool,
}

impl AppArmorProfile {
    /// Creates a new `AppArmorProfile` with a name and the path of the executable it protects.
    pub fn new(name: impl Into<String>, executable_path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            executable_path: executable_path.into(),
            paths: BTreeMap::new(),
            network_families: Vec::new(),
            disable_core_dumps: true,
        }
    }

    /// Adds fine-grained path permissions (e.g., "r", "w", "l", "px") for a specific file or directory path.
    pub fn add_path_permission(&mut self, path: impl Into<String>, permissions: impl Into<String>) -> &mut Self {
        self.paths.insert(path.into(), permissions.into());
        self
    }

    /// Adds an allowed network family rule (e.g., "inet", "inet6", "unix").
    pub fn add_network_family(&mut self, family: impl Into<String>) -> &mut Self {
        self.network_families.push(family.into());
        self
    }

    /// Sets whether to disable core dumps for this profile.
    pub fn set_disable_core_dumps(&mut self, disable: bool) -> &mut Self {
        self.disable_core_dumps = disable;
        self
    }

    /// Generates the AppArmor profile definition.
    pub fn generate_profile(&self) -> String {
        let mut out = String::new();

        let _ = writeln!(out, "abi <abi/3.0>,");
        let _ = writeln!(out, "");
        let _ = writeln!(out, "include <tunables/global>");
        let _ = writeln!(out, "");
        let _ = writeln!(out, "profile {} {} {{", self.name, self.executable_path);
        let _ = writeln!(out, "  include <abstractions/base>");

        if !self.network_families.is_empty() {
            let _ = writeln!(out, "");
            for family in &self.network_families {
                let _ = writeln!(out, "  network {},", family);
            }
        }

        if !self.paths.is_empty() {
            let _ = writeln!(out, "");
            for (path, perms) in &self.paths {
                let _ = writeln!(out, "  {} {},", path, perms);
            }
        }

        let _ = writeln!(out, "}}");

        out
    }

    /// Generates the required sysctl configuration to harden process core dumps.
    pub fn generate_sysctl_hardening(&self) -> Option<String> {
        if self.disable_core_dumps {
            Some("fs.suid_dumpable = 0\n".to_string())
        } else {
            None
        }
    }
}
