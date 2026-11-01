use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PermitRootLogin {
    Yes,
    ProhibitPassword,
    ForcedCommandsOnly,
    No,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SshHardeningConfig {
    pub permit_root_login: PermitRootLogin,
    pub password_authentication: bool,
    pub x11_forwarding: bool,
    pub allow_users: Vec<String>,
    pub listen_port: u16,
}

impl Default for SshHardeningConfig {
    fn default() -> Self {
        Self {
            permit_root_login: PermitRootLogin::No,
            password_authentication: false,
            x11_forwarding: false,
            allow_users: vec![],
            listen_port: 22,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SshError {
    #[error("Password authentication is enabled, which is insecure")]
    InsecurePasswordAuthentication,
    #[error("PermitRootLogin Yes is explicitly forbidden")]
    RootLoginAllowed,
    #[error("Listen port must be greater than 0")]
    InvalidPort,
    #[error("AllowUsers entries must be single safe OpenSSH user or user@host patterns")]
    InvalidAllowUser,
}

impl SshHardeningConfig {
    pub fn validate(&self) -> Result<(), SshError> {
        if self.permit_root_login == PermitRootLogin::Yes {
            return Err(SshError::RootLoginAllowed);
        }

        if self.password_authentication {
            return Err(SshError::InsecurePasswordAuthentication);
        }

        if self.listen_port == 0 {
            return Err(SshError::InvalidPort);
        }

        if self
            .allow_users
            .iter()
            .any(|user| !is_safe_allow_user_pattern(user))
        {
            return Err(SshError::InvalidAllowUser);
        }

        Ok(())
    }

    pub fn generate_sshd_config(&self) -> Result<String, SshError> {
        self.validate()?;

        let mut config = String::new();
        config.push_str(&format!("Port {}\n", self.listen_port));

        let prl_str = match self.permit_root_login {
            PermitRootLogin::Yes => "yes",
            PermitRootLogin::ProhibitPassword => "prohibit-password",
            PermitRootLogin::ForcedCommandsOnly => "forced-commands-only",
            PermitRootLogin::No => "no",
        };
        config.push_str(&format!("PermitRootLogin {}\n", prl_str));

        config.push_str(&format!(
            "PasswordAuthentication {}\n",
            if self.password_authentication {
                "yes"
            } else {
                "no"
            }
        ));
        // Keyboard-interactive authentication can still prompt for a password
        // even when PasswordAuthentication is disabled. Keep both password
        // based SSH methods off for this hardening profile.
        config.push_str("KbdInteractiveAuthentication no\n");
        config.push_str(&format!(
            "X11Forwarding {}\n",
            if self.x11_forwarding { "yes" } else { "no" }
        ));

        if !self.allow_users.is_empty() {
            config.push_str(&format!("AllowUsers {}\n", self.allow_users.join(" ")));
        }

        Ok(config)
    }
}

// Accept a conservative, whitespace-free subset of OpenSSH AllowUsers patterns.
// A pattern is either a username pattern or a username@host pattern. Keeping
// each value to one token prevents configuration directives from being injected.
fn is_safe_allow_user_pattern(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    let mut parts = value.split('@');
    let user = parts.next().unwrap_or_default();
    let host = parts.next();

    if parts.next().is_some() || user.is_empty() || host.is_some_and(|host| host.is_empty()) {
        return false;
    }

    let user_chars_are_safe = user
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '*' | '?'));
    let host_chars_are_safe = host.is_none_or(|host| {
        host.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '*' | '?' | ':'))
    });

    user_chars_are_safe && host_chars_are_safe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_hardening() {
        let config = SshHardeningConfig::default();
        assert!(config.validate().is_ok());
        let generated = config.generate_sshd_config().unwrap();
        assert!(generated.contains("PermitRootLogin no"));
        assert!(generated.contains("PasswordAuthentication no"));
        assert!(generated.contains("KbdInteractiveAuthentication no"));
    }

    #[test]
    fn test_insecure_password_auth() {
        let config = SshHardeningConfig {
            password_authentication: true,
            ..Default::default()
        };
        assert!(matches!(
            config.validate(),
            Err(SshError::InsecurePasswordAuthentication)
        ));
    }

    #[test]
    fn test_root_login_allowed() {
        let config = SshHardeningConfig {
            permit_root_login: PermitRootLogin::Yes,
            ..Default::default()
        };
        assert!(matches!(config.validate(), Err(SshError::RootLoginAllowed)));
    }

    #[test]
    fn rejects_allow_users_directive_injection() {
        let config = SshHardeningConfig {
            allow_users: vec!["alice\nPermitRootLogin yes".into()],
            ..Default::default()
        };

        assert!(matches!(config.validate(), Err(SshError::InvalidAllowUser)));
        assert!(matches!(
            config.generate_sshd_config(),
            Err(SshError::InvalidAllowUser)
        ));
    }

    #[test]
    fn rejects_allow_users_control_characters() {
        for value in ["alice\tadmin", "alice\rPermitRootLogin yes"] {
            let config = SshHardeningConfig {
                allow_users: vec![value.into()],
                ..Default::default()
            };
            assert!(matches!(config.validate(), Err(SshError::InvalidAllowUser)));
        }
    }

    #[test]
    fn accepts_safe_allow_users_patterns() {
        let config = SshHardeningConfig {
            allow_users: vec!["alice".into(), "deploy@10.0.0.*".into()],
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }
}
