#![forbid(unsafe_code)]

// Copyright 2026 FusionPanel Contributors
//
// This file is part of FusionPanel.
//
// FusionPanel is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// FusionPanel is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with FusionPanel.  If not, see <https://www.gnu.org/licenses/>.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustMode {
    /// Use system CA certificates for upstream verification
    SystemCa,
    /// Use internal CA certificates for upstream verification
    InternalCa { ca_cert_path: String },
    /// Explicitly disable verification. Must include audit and migration details.
    InsecureCompatibility {
        audit_trail: String,
        migration_path: String,
    },
}

impl Default for TrustMode {
    fn default() -> Self {
        Self::SystemCa
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamTlsPolicy {
    pub trust_mode: TrustMode,
    pub expected_hostname: String,
    pub sni_preservation: bool,
}

impl UpstreamTlsPolicy {
    pub fn new(expected_hostname: impl Into<String>) -> Self {
        Self {
            trust_mode: TrustMode::default(),
            expected_hostname: expected_hostname.into(),
            sni_preservation: true, // Default to preserving SNI
        }
    }

    pub fn with_trust_mode(mut self, trust_mode: TrustMode) -> Self {
        self.trust_mode = trust_mode;
        self
    }

    pub fn with_sni_preservation(mut self, preserve: bool) -> Self {
        self.sni_preservation = preserve;
        self
    }

    pub fn generate_nginx_config(&self) -> String {
        let mut config = String::new();
        config.push_str("proxy_ssl_server_name ");
        config.push_str(if self.sni_preservation {
            "on;\n"
        } else {
            "off;\n"
        });

        if self.sni_preservation {
            config.push_str(&format!("proxy_ssl_name {};\n", self.expected_hostname));
        }

        match &self.trust_mode {
            TrustMode::SystemCa => {
                config.push_str("proxy_ssl_verify on;\n");
                config.push_str(
                    "proxy_ssl_trusted_certificate /etc/ssl/certs/ca-certificates.crt;\n",
                );
                config.push_str("proxy_ssl_verify_depth 2;\n");
            }
            TrustMode::InternalCa { ca_cert_path } => {
                config.push_str("proxy_ssl_verify on;\n");
                config.push_str(&format!(
                    "proxy_ssl_trusted_certificate {};\n",
                    ca_cert_path
                ));
                config.push_str("proxy_ssl_verify_depth 2;\n");
            }
            TrustMode::InsecureCompatibility {
                audit_trail,
                migration_path,
            } => {
                config.push_str("# WARNING: Insecure compatibility mode enabled\n");
                config.push_str(&format!("# Audit Trail: {}\n", audit_trail));
                config.push_str(&format!("# Migration Path: {}\n", migration_path));
                config.push_str("proxy_ssl_verify off;\n");
            }
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mode_is_system_ca() {
        let policy = UpstreamTlsPolicy::new("example.com");
        assert_eq!(policy.trust_mode, TrustMode::SystemCa);
        assert_eq!(policy.sni_preservation, true);
    }

    #[test]
    fn test_nginx_config_system_ca() {
        let policy = UpstreamTlsPolicy::new("backend.internal");
        let config = policy.generate_nginx_config();

        assert!(config.contains("proxy_ssl_server_name on;"));
        assert!(config.contains("proxy_ssl_name backend.internal;"));
        assert!(config.contains("proxy_ssl_verify on;"));
        assert!(
            config.contains("proxy_ssl_trusted_certificate /etc/ssl/certs/ca-certificates.crt;")
        );
    }

    #[test]
    fn test_nginx_config_internal_ca() {
        let policy =
            UpstreamTlsPolicy::new("service.local").with_trust_mode(TrustMode::InternalCa {
                ca_cert_path: "/etc/fusionpanel/ssl/internal-ca.crt".to_string(),
            });

        let config = policy.generate_nginx_config();

        assert!(config.contains("proxy_ssl_server_name on;"));
        assert!(config.contains("proxy_ssl_name service.local;"));
        assert!(config.contains("proxy_ssl_verify on;"));
        assert!(
            config.contains("proxy_ssl_trusted_certificate /etc/fusionpanel/ssl/internal-ca.crt;")
        );
    }

    #[test]
    fn test_nginx_config_insecure_mode() {
        let policy = UpstreamTlsPolicy::new("legacy.app").with_trust_mode(
            TrustMode::InsecureCompatibility {
                audit_trail: "TICKET-1234: Temporary override for legacy application".to_string(),
                migration_path: "Application to be upgraded by Q4 2026".to_string(),
            },
        );

        let config = policy.generate_nginx_config();

        assert!(config.contains("proxy_ssl_server_name on;"));
        assert!(config.contains("proxy_ssl_name legacy.app;"));
        assert!(config.contains("proxy_ssl_verify off;"));
        assert!(config.contains("# WARNING: Insecure compatibility mode enabled"));
        assert!(config
            .contains("# Audit Trail: TICKET-1234: Temporary override for legacy application"));
        assert!(config.contains("# Migration Path: Application to be upgraded by Q4 2026"));
    }

    #[test]
    fn test_nginx_config_no_sni_preservation() {
        let policy = UpstreamTlsPolicy::new("backend.internal").with_sni_preservation(false);

        let config = policy.generate_nginx_config();

        assert!(config.contains("proxy_ssl_server_name off;"));
        assert!(!config.contains("proxy_ssl_name"));
    }
}
