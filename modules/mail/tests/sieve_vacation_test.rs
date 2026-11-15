#![forbid(unsafe_code)]

use chrono::{TimeZone, Utc};
use mail::error::MailError;
use mail::sieve_vacation::{SieveVacationConfig, SieveVacationGenerator};

#[test]
fn test_vacation_generator_empty_addresses() {
    let config = SieveVacationConfig {
        days: 7,
        subject: Some("Out of Office".to_string()),
        addresses: vec![],
        message: "I am out".to_string(),
        start_time: None,
        end_time: None,
    };

    let result = SieveVacationGenerator::generate(&config);
    assert!(result.is_err());
    if let Err(MailError::InvalidInput(msg)) = result {
        assert!(msg.contains("At least one address must be provided"));
    } else {
        panic!("Expected InvalidInput error");
    }
}

#[test]
fn test_vacation_generator_basic() {
    let config = SieveVacationConfig {
        days: 7,
        subject: Some("Out of Office".to_string()),
        addresses: vec!["me@example.com".to_string()],
        message: "I am out".to_string(),
        start_time: None,
        end_time: None,
    };

    let result = SieveVacationGenerator::generate(&config).unwrap();

    assert!(result.contains("require [\"vacation\", \"regex\"];")); // since Precedence uses regex
    assert!(result.contains("if allof (not anyof (header :is \"Auto-Submitted\" \"auto-replied\", header :is \"Auto-Submitted\" \"auto-generated\", header :regex \"Precedence\" \"^(bulk|junk)$\", header :matches \"List-Id\" \"*\", header :matches \"X-Cron-Env\" \"*\")) {"));
    assert!(result.contains("    vacation :days 7 :subject \"Out of Office\" :addresses [\"me@example.com\"] \"I am out\";"));
}

#[test]
fn test_vacation_generator_days_clamping() {
    let mut config = SieveVacationConfig {
        days: 100, // Should be clamped to 30
        subject: None,
        addresses: vec!["test@example.com".to_string()],
        message: "Msg".to_string(),
        start_time: None,
        end_time: None,
    };

    let result = SieveVacationGenerator::generate(&config).unwrap();
    assert!(result.contains("vacation :days 30"));

    config.days = 0; // Should be clamped to 1
    let result = SieveVacationGenerator::generate(&config).unwrap();
    assert!(result.contains("vacation :days 1"));
}

#[test]
fn test_vacation_generator_dates() {
    let start_time = Utc.with_ymd_and_hms(2023, 10, 1, 0, 0, 0).unwrap();
    let end_time = Utc.with_ymd_and_hms(2023, 10, 10, 0, 0, 0).unwrap();

    let config = SieveVacationConfig {
        days: 7,
        subject: Some("Out of Office".to_string()),
        addresses: vec!["me@example.com".to_string()],
        message: "I am out".to_string(),
        start_time: Some(start_time),
        end_time: Some(end_time),
    };

    let result = SieveVacationGenerator::generate(&config).unwrap();

    assert!(result.contains("require [\"vacation\", \"regex\", \"date\", \"relational\"];"));
    assert!(result.contains("currentdate :zone \"+0000\" :value \"ge\" \"date\" \"2023-10-01\""));
    assert!(result.contains("currentdate :zone \"+0000\" :value \"le\" \"date\" \"2023-10-10\""));
}

#[test]
fn test_vacation_generator_multi_addresses() {
    let config = SieveVacationConfig {
        days: 7,
        subject: Some("OOF".to_string()),
        addresses: vec![
            "me@example.com".to_string(),
            "alias@example.com".to_string(),
        ],
        message: "OOF message".to_string(),
        start_time: None,
        end_time: None,
    };

    let result = SieveVacationGenerator::generate(&config).unwrap();
    assert!(result.contains(":addresses [\"me@example.com\", \"alias@example.com\"]"));
}
