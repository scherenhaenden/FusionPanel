#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Container runs in privileged mode")]
    PrivilegedContainer,
    #[error("Container has unconfined AppArmor profile")]
    UnconfinedAppArmor,
    #[error("Container has dangerous capability: {0}")]
    DangerousCapability(String),
    #[error("Seccomp is disabled")]
    SeccompDisabled,
}

#[derive(Debug, Clone, Default)]
pub struct ContainerSpec {
    pub name: String,
    pub privileged: bool,
    pub apparmor_profile: Option<String>,
    pub seccomp_profile: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AuditResult {
    pub score: u8,
    pub violations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GeneratedProfile {
    pub profile_name: String,
    pub profile_content: String,
}

#[derive(Debug, Default)]
pub struct RuntimeSecurityProfileManager;

impl RuntimeSecurityProfileManager {
    pub fn new() -> Self {
        Self
    }

    /// Validates a container specification against dangerous configurations.
    pub fn validate_spec(&self, spec: &ContainerSpec) -> Result<(), SecurityError> {
        if spec.privileged {
            return Err(SecurityError::PrivilegedContainer);
        }

        if let Some(profile) = &spec.apparmor_profile {
            if profile == "unconfined" {
                return Err(SecurityError::UnconfinedAppArmor);
            }
        }

        if let Some(seccomp) = &spec.seccomp_profile {
            if seccomp == "unconfined" {
                return Err(SecurityError::SeccompDisabled);
            }
        } else {
            // Treating absence of seccomp profile as disabled, depending on strictness
            return Err(SecurityError::SeccompDisabled);
        }

        for cap in &spec.capabilities {
            if cap == "CAP_SYS_ADMIN" || cap == "SYS_ADMIN" {
                return Err(SecurityError::DangerousCapability(
                    "CAP_SYS_ADMIN".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Provides a security compliance auditing score (0 to 100) for a container spec.
    pub fn audit_score(&self, spec: &ContainerSpec) -> AuditResult {
        let mut score: u8 = 100;
        let mut violations = Vec::new();

        if spec.privileged {
            score = score.saturating_sub(100);
            violations.push("Privileged mode is enabled".to_string());
        }

        match &spec.apparmor_profile {
            Some(p) if p == "unconfined" => {
                score = score.saturating_sub(30);
                violations.push("AppArmor profile is unconfined".to_string());
            }
            None => {
                score = score.saturating_sub(10);
                violations.push("No AppArmor profile explicitly set".to_string());
            }
            _ => {}
        }

        match &spec.seccomp_profile {
            Some(p) if p == "unconfined" => {
                score = score.saturating_sub(30);
                violations.push("Seccomp is disabled (unconfined)".to_string());
            }
            None => {
                score = score.saturating_sub(10);
                violations.push("No seccomp profile explicitly set".to_string());
            }
            _ => {}
        }

        for cap in &spec.capabilities {
            if cap == "CAP_SYS_ADMIN" || cap == "SYS_ADMIN" {
                score = score.saturating_sub(50);
                violations.push(format!("Dangerous capability granted: {}", cap));
            } else {
                score = score.saturating_sub(5);
                violations.push(format!("Additional capability granted: {}", cap));
            }
        }

        AuditResult { score, violations }
    }

    /// Generates container profile definitions enforcing restricted filesystem access,
    /// forbidding raw sockets and ptrace attachment.
    pub fn generate_profile(&self, spec: &ContainerSpec) -> GeneratedProfile {
        let profile_name = format!("docker-fusionpanel-{}", spec.name);

        let profile_content = format!(
            r#"#include <tunables/global>

profile {profile_name} flags=(attach_disconnected,mediate_deleted) {{
  #include <abstractions/base>

  # Network restrictions
  network inet tcp,
  network inet udp,
  network inet icmp,
  deny network raw,
  deny network packet,

  # Deny ptrace attachment
  deny ptrace,

  # File system restrictions
  deny /proc/kcore rwklx,
  deny /sys/firmware/** rwklx,

  # Base file access
  file,
}}
"#
        );

        GeneratedProfile {
            profile_name,
            profile_content,
        }
    }

    /// Attaches the generated security profile to the container spec.
    pub fn attach_profile(&self, spec: &mut ContainerSpec) {
        let profile = self.generate_profile(spec);
        spec.apparmor_profile = Some(profile.profile_name);
    }
}
