#![forbid(unsafe_code)]

use mail::delivery::MaildirDelivery;
use mail::domain::MailAddress;
use std::net::TcpStream;
use std::time::Duration;
use tempfile::TempDir;

/// Mailbox-to-mailbox local message delivery and retrieval.
/// Verifies the local delivery path from sending mailbox spool to recipient mailbox retrieval.
#[test]
fn test_e2e_mailbox_to_mailbox_local_delivery() {
    let tmp = TempDir::new().unwrap();
    let delivery = MaildirDelivery::new(tmp.path());

    let mailbox_a = MailAddress::parse("sender@example.com").unwrap();
    let mailbox_b = MailAddress::parse("recipient@example.com").unwrap();

    delivery.init_mailbox(&mailbox_a).unwrap();
    delivery.init_mailbox(&mailbox_b).unwrap();

    let body = b"Subject: Direct Mailbox Delivery\r\n\r\nHello from mailbox A to mailbox B.";
    let msg_id = delivery
        .deliver_message(&mailbox_b, &mailbox_a, body)
        .unwrap();

    let b_messages = delivery.list_messages(&mailbox_b, "new").unwrap();
    assert_eq!(b_messages, vec![msg_id.clone()]);

    let retrieved = delivery.read_message(&mailbox_b, "new", &msg_id).unwrap();
    let s = String::from_utf8_lossy(&retrieved);
    assert!(s.contains("Hello from mailbox A to mailbox B."));
    assert!(s.contains("Delivered-To: <recipient@example.com>"));
    assert!(s.contains("Return-Path: <sender@example.com>"));
}

/// End-to-end SMTP-to-IMAP live delivery gate.
/// Explicitly records BLOCKED when live SMTP/IMAP servers are not present in test environment.
#[test]
#[ignore = "BLOCKED: Real SMTP/IMAP daemon or disposable container stack (e.g. postfix/dovecot on ports 2525/1143) is not available. Dependency missing."]
fn test_e2e_smtp_imap_live_network_delivery_blocked() {
    let smtp_host = std::env::var("FUSIONPANEL_TEST_SMTP_HOST")
        .unwrap_or_else(|_| "127.0.0.1:2525".to_string());
    let imap_host = std::env::var("FUSIONPANEL_TEST_IMAP_HOST")
        .unwrap_or_else(|_| "127.0.0.1:1143".to_string());

    let smtp_conn = TcpStream::connect_timeout(
        &smtp_host
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:2525".parse().unwrap()),
        Duration::from_millis(500),
    );
    let imap_conn = TcpStream::connect_timeout(
        &imap_host
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:1143".parse().unwrap()),
        Duration::from_millis(500),
    );

    if smtp_conn.is_err() || imap_conn.is_err() {
        panic!("BLOCKED: Live network delivery requires running SMTP service at {smtp_host} and IMAP service at {imap_host}");
    }
}
