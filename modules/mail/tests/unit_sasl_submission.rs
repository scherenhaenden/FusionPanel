#![forbid(unsafe_code)]

use mail::sasl_submission::SaslSubmissionConfig;

#[test]
fn test_default_config() {
    let config = SaslSubmissionConfig::default();
    assert_eq!(config.submission_port, 587);
    assert_eq!(config.smtps_port, 465);
    assert_eq!(config.smtpd_sasl_type, "dovecot");
    assert_eq!(config.smtpd_sasl_path, "private/auth");
    assert_eq!(config.smtpd_tls_security_level, "encrypt");
    assert_eq!(config.smtpd_client_restrictions, vec!["permit_sasl_authenticated", "reject"]);
}

#[test]
fn test_generate_master_cf_default() {
    let config = SaslSubmissionConfig::default();
    let master_cf = config.generate_master_cf();

    assert!(master_cf.contains("587 inet n - y - - smtpd"));
    assert!(master_cf.contains("-o syslog_name=postfix/submission"));
    assert!(master_cf.contains("-o smtpd_tls_security_level=encrypt"));
    assert!(master_cf.contains("-o smtpd_sasl_auth_enable=yes"));
    assert!(master_cf.contains("-o smtpd_sasl_type=dovecot"));
    assert!(master_cf.contains("-o smtpd_sasl_path=private/auth"));
    assert!(master_cf.contains("-o smtpd_client_restrictions=permit_sasl_authenticated,reject"));

    assert!(master_cf.contains("465 inet n - y - - smtpd"));
    assert!(master_cf.contains("-o syslog_name=postfix/smtps"));
    assert!(master_cf.contains("-o smtpd_tls_wrappermode=yes"));
    assert!(master_cf.contains("-o smtpd_sasl_auth_enable=yes"));
    assert!(master_cf.contains("-o smtpd_sasl_type=dovecot"));
    assert!(master_cf.contains("-o smtpd_sasl_path=private/auth"));
    assert!(master_cf.contains("-o smtpd_client_restrictions=permit_sasl_authenticated,reject"));
}

#[test]
fn test_generate_master_cf_custom() {
    let config = SaslSubmissionConfig {
        submission_port: 5587,
        smtps_port: 5465,
        smtpd_sasl_type: "cyrus".to_string(),
        smtpd_sasl_path: "smtpd".to_string(),
        smtpd_tls_security_level: "may".to_string(),
        smtpd_client_restrictions: vec!["permit_mynetworks".to_string(), "reject".to_string()],
    };
    let master_cf = config.generate_master_cf();

    assert!(master_cf.contains("5587 inet n - y - - smtpd"));
    assert!(master_cf.contains("-o smtpd_tls_security_level=may"));
    assert!(master_cf.contains("-o smtpd_sasl_type=cyrus"));
    assert!(master_cf.contains("-o smtpd_sasl_path=smtpd"));
    assert!(master_cf.contains("-o smtpd_client_restrictions=permit_mynetworks,reject"));

    assert!(master_cf.contains("5465 inet n - y - - smtpd"));
    assert!(master_cf.contains("-o smtpd_tls_wrappermode=yes"));
    assert!(master_cf.contains("-o smtpd_sasl_type=cyrus"));
    assert!(master_cf.contains("-o smtpd_sasl_path=smtpd"));
    assert!(master_cf.contains("-o smtpd_client_restrictions=permit_mynetworks,reject"));
}
