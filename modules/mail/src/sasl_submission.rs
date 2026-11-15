#![forbid(unsafe_code)]

/// Configuration for Postfix SASL Submission and SMTPS services
#[derive(Debug, Clone)]
pub struct SaslSubmissionConfig {
    pub submission_port: u16,
    pub smtps_port: u16,
    pub smtpd_sasl_type: String,
    pub smtpd_sasl_path: String,
    pub smtpd_tls_security_level: String,
    pub smtpd_client_restrictions: Vec<String>,
}

impl Default for SaslSubmissionConfig {
    fn default() -> Self {
        Self {
            submission_port: 587,
            smtps_port: 465,
            smtpd_sasl_type: "dovecot".to_string(),
            smtpd_sasl_path: "private/auth".to_string(),
            smtpd_tls_security_level: "encrypt".to_string(),
            smtpd_client_restrictions: vec![
                "permit_sasl_authenticated".to_string(),
                "reject".to_string(),
            ],
        }
    }
}

impl SaslSubmissionConfig {
    /// Generate the master.cf configuration block for submission and smtps
    pub fn generate_master_cf(&self) -> String {
        let mut config = String::new();
        let client_restrictions = self.smtpd_client_restrictions.join(",");

        // Submission service (587)
        config.push_str(&format!("{} inet n - y - - smtpd\n", self.submission_port));
        config.push_str("  -o syslog_name=postfix/submission\n");
        config.push_str(&format!("  -o smtpd_tls_security_level={}\n", self.smtpd_tls_security_level));
        config.push_str("  -o smtpd_sasl_auth_enable=yes\n");
        config.push_str(&format!("  -o smtpd_sasl_type={}\n", self.smtpd_sasl_type));
        config.push_str(&format!("  -o smtpd_sasl_path={}\n", self.smtpd_sasl_path));
        config.push_str(&format!("  -o smtpd_client_restrictions={}\n", client_restrictions));
        config.push_str("  -o smtpd_relay_restrictions=permit_sasl_authenticated,reject\n");
        config.push_str("  -o milter_macro_daemon_name=ORIGINATING\n");

        config.push('\n');

        // SMTPS service (465)
        config.push_str(&format!("{} inet n - y - - smtpd\n", self.smtps_port));
        config.push_str("  -o syslog_name=postfix/smtps\n");
        config.push_str("  -o smtpd_tls_wrappermode=yes\n");
        config.push_str("  -o smtpd_sasl_auth_enable=yes\n");
        config.push_str(&format!("  -o smtpd_sasl_type={}\n", self.smtpd_sasl_type));
        config.push_str(&format!("  -o smtpd_sasl_path={}\n", self.smtpd_sasl_path));
        config.push_str(&format!("  -o smtpd_client_restrictions={}\n", client_restrictions));
        config.push_str("  -o smtpd_relay_restrictions=permit_sasl_authenticated,reject\n");
        config.push_str("  -o milter_macro_daemon_name=ORIGINATING\n");

        config
    }
}
