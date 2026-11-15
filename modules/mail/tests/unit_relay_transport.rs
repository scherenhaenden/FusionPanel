#![forbid(unsafe_code)]

use mail::relay_transport::{
    RelayConfig, RelayHost, SenderRelayEntry, TransportEntry, TransportMethod,
};

#[test]
fn test_transport_method_display() {
    assert_eq!(TransportMethod::Smtp.to_string(), "smtp");
    assert_eq!(TransportMethod::Lmtp.to_string(), "lmtp");
    assert_eq!(TransportMethod::Local.to_string(), "local");
    assert_eq!(
        TransportMethod::Error("mail for domain.com is not deliverable".to_string()).to_string(),
        "error:mail for domain.com is not deliverable"
    );
    assert_eq!(TransportMethod::Discard.to_string(), "discard");
    assert_eq!(
        TransportMethod::Custom("uucp".to_string()).to_string(),
        "uucp"
    );
}

#[test]
fn test_relay_host_display() {
    let host1 = RelayHost::new("smtp.sendgrid.net", Some(587), false);
    assert_eq!(host1.to_string(), "[smtp.sendgrid.net]:587");

    let host2 = RelayHost::new("smtp.sendgrid.net", None, false);
    assert_eq!(host2.to_string(), "[smtp.sendgrid.net]");

    let host3 = RelayHost::new("example.com", Some(25), true);
    assert_eq!(host3.to_string(), "example.com:25");

    let host4 = RelayHost::new("example.com", None, true);
    assert_eq!(host4.to_string(), "example.com");
}

#[test]
fn test_transport_entry_display() {
    let entry1 = TransportEntry::new(
        "example.com".to_string(),
        TransportMethod::Smtp,
        Some(RelayHost::new("smtp.sendgrid.net", Some(587), false)),
    );
    assert_eq!(
        entry1.to_string(),
        "example.com\tsmtp:[smtp.sendgrid.net]:587"
    );

    let entry2 = TransportEntry::new("example.com".to_string(), TransportMethod::Local, None);
    assert_eq!(entry2.to_string(), "example.com\tlocal");

    let entry3 = TransportEntry::new(
        "*@example.com".to_string(),
        TransportMethod::Error("bounce".to_string()),
        None,
    );
    assert_eq!(entry3.to_string(), "*@example.com\terror:bounce");
}

#[test]
fn test_sender_relay_entry_display() {
    let entry = SenderRelayEntry::new(
        "@example.com".to_string(),
        RelayHost::new("smtp.mailgun.org", Some(587), false),
    );
    assert_eq!(entry.to_string(), "@example.com\t[smtp.mailgun.org]:587");
}

#[test]
fn test_relay_config_generate_maps() {
    let mut config = RelayConfig::new();

    config.add_transport(TransportEntry::new(
        "example.com".to_string(),
        TransportMethod::Smtp,
        Some(RelayHost::new("smtp.sendgrid.net", Some(587), false)),
    ));
    config.add_transport(TransportEntry::new(
        "local.domain".to_string(),
        TransportMethod::Local,
        None,
    ));

    config.add_sender_relay(SenderRelayEntry::new(
        "@example.com".to_string(),
        RelayHost::new("smtp.mailgun.org", Some(587), false),
    ));
    config.add_sender_relay(SenderRelayEntry::new(
        "user@example.org".to_string(),
        RelayHost::new("email-smtp.us-east-1.amazonaws.com", Some(465), false),
    ));

    let transport_map = config.generate_transport_map();
    let expected_transport_map = "\
# Generated Postfix transport map
example.com\tsmtp:[smtp.sendgrid.net]:587
local.domain\tlocal
";
    assert_eq!(transport_map, expected_transport_map);

    let sender_relay_map = config.generate_sender_relay_map();
    let expected_sender_relay_map = "\
# Generated Postfix sender-dependent relayhost map
@example.com\t[smtp.mailgun.org]:587
user@example.org\t[email-smtp.us-east-1.amazonaws.com]:465
";
    assert_eq!(sender_relay_map, expected_sender_relay_map);
}
