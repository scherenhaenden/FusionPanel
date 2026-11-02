use crate::delivery::MaildirDelivery;
use crate::domain::{MailAddress, MailDomainName};
use crate::error::MailError;
use crate::models::{
    AuthMode, MailAlias, MailDomain, Mailbox, MailboxCredentials, RedactedSecret, SmtpRelayConfig,
};
use crate::plan::MailConfigPlan;
use uuid::Uuid;

#[test]
fn test_domain_name_valid() {
    let name = MailDomainName::parse("example.com").unwrap();
    assert_eq!(name.as_str(), "example.com");

    let name2 = MailDomainName::parse("mail.sub-domain.example.org.").unwrap();
    assert_eq!(name2.as_str(), "mail.sub-domain.example.org");

    let puny = MailDomainName::parse("xn--bcher-kva.ch").unwrap();
    assert_eq!(puny.as_str(), "xn--bcher-kva.ch");
}

#[test]
fn test_domain_name_invalid_cases() {
    // Empty
    assert!(matches!(
        MailDomainName::parse(""),
        Err(MailError::InvalidInput(_))
    ));
    // Unicode rejected
    assert!(matches!(
        MailDomainName::parse("münchen.de"),
        Err(MailError::InvalidInput(_))
    ));
    // IP literals rejected
    assert!(matches!(
        MailDomainName::parse("192.168.1.1"),
        Err(MailError::InvalidInput(_))
    ));
    assert!(matches!(
        MailDomainName::parse("2001:db8::1"),
        Err(MailError::InvalidInput(_))
    ));
    // Double dots / empty labels
    assert!(matches!(
        MailDomainName::parse("example..com"),
        Err(MailError::InvalidInput(_))
    ));
    // Leading / trailing hyphens
    assert!(matches!(
        MailDomainName::parse("-example.com"),
        Err(MailError::InvalidInput(_))
    ));
    assert!(matches!(
        MailDomainName::parse("example-.com"),
        Err(MailError::InvalidInput(_))
    ));
    // Exceeds label limit (63 chars)
    let long_label = format!("{}.com", "a".repeat(64));
    assert!(matches!(
        MailDomainName::parse(&long_label),
        Err(MailError::InvalidInput(_))
    ));
}

#[test]
fn test_mail_address_validation() {
    let addr = MailAddress::parse("user.name+tag@example.com").unwrap();
    assert_eq!(addr.local_part(), "user.name+tag");
    assert_eq!(addr.domain().as_str(), "example.com");
    assert_eq!(addr.full_address(), "user.name+tag@example.com");

    // Negative cases
    assert!(matches!(
        MailAddress::parse("notanaddress"),
        Err(MailError::InvalidInput(_))
    ));
    assert!(matches!(
        MailAddress::parse("@example.com"),
        Err(MailError::InvalidInput(_))
    ));
    assert!(matches!(
        MailAddress::parse("user@"),
        Err(MailError::InvalidInput(_))
    ));
    assert!(matches!(
        MailAddress::parse(".user@example.com"),
        Err(MailError::InvalidInput(_))
    ));
    assert!(matches!(
        MailAddress::parse("user.@example.com"),
        Err(MailError::InvalidInput(_))
    ));
    assert!(matches!(
        MailAddress::parse("user..name@example.com"),
        Err(MailError::InvalidInput(_))
    ));
    // Injection attempts
    assert!(matches!(
        MailAddress::parse("user\nname@example.com"),
        Err(MailError::InvalidInput(_))
    ));
    assert!(matches!(
        MailAddress::parse("user;name@example.com"),
        Err(MailError::InvalidInput(_))
    ));
}

#[test]
fn test_redacted_secret_preserves_privacy() {
    let secret = RedactedSecret::new("super_secret_password_123").unwrap();
    let debug_str = format!("{:?}", secret);
    let display_str = format!("{}", secret);
    let serialized_str = serde_json::to_string(&secret).unwrap();

    assert_eq!(debug_str, "[REDACTED]");
    assert_eq!(display_str, "[REDACTED]");
    assert_eq!(serialized_str, "\"[REDACTED]\"");
    assert_eq!(secret.expose_secret(), "super_secret_password_123");

    // Null and newline injection in secrets
    assert!(matches!(
        RedactedSecret::new("pass\nword"),
        Err(MailError::InvalidInput(_))
    ));
    assert!(matches!(
        RedactedSecret::new("pass\0word"),
        Err(MailError::InvalidInput(_))
    ));
}

#[test]
fn test_mailbox_credentials_and_serialization() {
    let secret = RedactedSecret::new("vault_secret").unwrap();
    let creds = MailboxCredentials {
        mailbox_id: Uuid::new_v4(),
        password_hash: secret,
        auth_mode: AuthMode::DovecotSha512Crypt,
    };
    let json = serde_json::to_string(&creds).unwrap();
    assert!(!json.contains("vault_secret"));
    assert!(json.contains("[REDACTED]"));
}

#[test]
fn test_config_plan_generation_and_injection_prevention() {
    let dom = MailDomain::new(
        MailDomainName::parse("example.com").unwrap(),
        Uuid::new_v4(),
    );
    let addr = MailAddress::parse("admin@example.com").unwrap();
    let mb = Mailbox::new(dom.id, addr.clone(), 1024 * 1024 * 100).unwrap();

    let fwd_addr = MailAddress::parse("backup@example.com").unwrap();
    let alias = MailAlias::new(addr, vec![fwd_addr]).unwrap();

    let relay = SmtpRelayConfig::new("smtp.relayprovider.net", 587).unwrap();

    let plan = MailConfigPlan::generate(&[dom], &[mb], &[alias], Some(&relay)).unwrap();

    assert_eq!(
        plan.virtual_mailbox_domains,
        vec!["example.com OK".to_string()]
    );
    assert_eq!(
        plan.virtual_mailbox_maps,
        vec!["admin@example.com example.com/admin/".to_string()]
    );
    assert_eq!(
        plan.virtual_alias_maps,
        vec!["admin@example.com backup@example.com".to_string()]
    );
    assert_eq!(
        plan.relay_host_directive,
        Some("[smtp.relayprovider.net]:587".to_string())
    );
}

#[test]
fn test_maildir_delivery_and_retrieval() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let delivery = MaildirDelivery::new(tmp_dir.path());

    let recipient = MailAddress::parse("alice@example.com").unwrap();
    let sender = MailAddress::parse("bob@example.com").unwrap();

    delivery.init_mailbox(&recipient).unwrap();

    // Verify directories exist
    let base = delivery.mailbox_dir(&recipient).unwrap();
    assert!(base.join("new").exists());
    assert!(base.join("tmp").exists());
    assert!(base.join("cur").exists());

    let message_body = b"Subject: Hello Alice\r\n\r\nThis is a test message.";
    let msg_file = delivery
        .deliver_message(&recipient, &sender, message_body)
        .unwrap();

    let new_msgs = delivery.list_messages(&recipient, "new").unwrap();
    assert_eq!(new_msgs.len(), 1);
    assert_eq!(new_msgs[0], msg_file);

    let content = delivery.read_message(&recipient, "new", &msg_file).unwrap();
    let content_str = String::from_utf8_lossy(&content);
    assert!(content_str.contains("Delivered-To: <alice@example.com>"));
    assert!(content_str.contains("Return-Path: <bob@example.com>"));
    assert!(content_str.contains("This is a test message."));
}

#[test]
fn test_maildir_path_traversal_rejection() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let delivery = MaildirDelivery::new(tmp_dir.path());
    let recipient = MailAddress::parse("alice@example.com").unwrap();

    assert!(delivery
        .read_message(&recipient, "new", "../../../etc/passwd")
        .is_err());
    assert!(delivery
        .read_message(&recipient, "invalid_folder", "msg")
        .is_err());
}
