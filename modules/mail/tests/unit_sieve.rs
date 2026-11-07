#![forbid(unsafe_code)]

use mail::sieve::{MatchType, SieveAction, SieveCondition, SieveRule, SieveScript};

#[test]
fn test_sieve_script_generation_empty() {
    let script = SieveScript::new();
    let generated = script.generate().unwrap();
    assert_eq!(generated, "");
}

#[test]
fn test_sieve_script_generation_fileinto() {
    let mut script = SieveScript::new();
    script.add_rule(SieveRule {
        name: Some("Spam Filter".to_string()),
        condition: SieveCondition::Header {
            header: "X-Spam-Flag".to_string(),
            match_type: MatchType::Is,
            value: "YES".to_string(),
        },
        actions: vec![SieveAction::FileInto("Junk".to_string())],
    });

    let generated = script.generate().unwrap();
    assert!(generated.contains("require [\"fileinto\"];"));
    assert!(generated.contains("# Rule: Spam Filter"));
    assert!(generated.contains("if header :is \"X-Spam-Flag\" \"YES\" {"));
    assert!(generated.contains("    fileinto \"Junk\";"));
}

#[test]
fn test_sieve_script_generation_vacation() {
    let mut script = SieveScript::new();
    script.add_rule(SieveRule {
        name: Some("Auto Reply".to_string()),
        condition: SieveCondition::True,
        actions: vec![SieveAction::Vacation {
            days: Some(7),
            subject: Some("Out of office".to_string()),
            addresses: vec!["me@example.com".to_string()],
            message: "I am out of the office until next week.\nThanks.".to_string(),
        }],
    });

    let generated = script.generate().unwrap();
    assert!(generated.contains("require [\"vacation\"];"));
    assert!(generated.contains("if true {"));
    assert!(generated.contains(
        "vacation :days 7 :subject \"Out of office\" :addresses [\"me@example.com\"] text:"
    ));
    assert!(generated.contains("I am out of the office until next week.\nThanks.\n.\n;"));
}

#[test]
fn test_sieve_dot_stuffing() {
    let mut script = SieveScript::new();
    script.add_rule(SieveRule {
        name: None,
        condition: SieveCondition::True,
        actions: vec![SieveAction::Vacation {
            days: None,
            subject: None,
            addresses: vec![],
            message: "Hello,\n.This line starts with a dot.\nBye".to_string(),
        }],
    });

    let generated = script.generate().unwrap();
    assert!(generated.contains("Hello,\n..This line starts with a dot.\nBye\n.\n;"));
}

#[test]
fn test_sieve_script_multiple_rules() {
    let mut script = SieveScript::new();

    script.add_rule(SieveRule {
        name: Some("Block bad words".to_string()),
        condition: SieveCondition::Header {
            header: "Subject".to_string(),
            match_type: MatchType::Contains,
            value: "viagra".to_string(),
        },
        actions: vec![SieveAction::Discard],
    });

    script.add_rule(SieveRule {
        name: Some("Forward Important".to_string()),
        condition: SieveCondition::Header {
            header: "From".to_string(),
            match_type: MatchType::Is,
            value: "boss@example.com".to_string(),
        },
        actions: vec![
            SieveAction::RedirectCopy("myother@example.com".to_string()),
            SieveAction::Keep,
        ],
    });

    let generated = script.generate().unwrap();
    assert!(generated.contains("require [\"copy\"];"));
    assert!(generated.contains("if header :contains \"Subject\" \"viagra\" {"));
    assert!(generated.contains("    discard;"));
    assert!(generated.contains("if header :is \"From\" \"boss@example.com\" {"));
    assert!(generated.contains("    redirect :copy \"myother@example.com\";"));
    assert!(generated.contains("    keep;"));
}

#[test]
fn test_sieve_complex_condition() {
    let mut script = SieveScript::new();
    script.add_rule(SieveRule {
        name: None,
        condition: SieveCondition::AllOf(vec![
            SieveCondition::SizeOver(1024),
            SieveCondition::AnyOf(vec![
                SieveCondition::Header {
                    header: "Subject".to_string(),
                    match_type: MatchType::Regex,
                    value: "^test".to_string(),
                },
                SieveCondition::Header {
                    header: "To".to_string(),
                    match_type: MatchType::Matches,
                    value: "*@test.com".to_string(),
                },
            ]),
        ]),
        actions: vec![SieveAction::Reject("Too large and testy".to_string())],
    });

    let generated = script.generate().unwrap();
    assert!(generated.contains("require [\"reject\", \"regex\"];"));
    assert!(generated.contains("if allof (size :over 1024, anyof (header :regex \"Subject\" \"^test\", header :matches \"To\" \"*@test.com\")) {"));
    assert!(generated.contains("    reject \"Too large and testy\";"));
}
