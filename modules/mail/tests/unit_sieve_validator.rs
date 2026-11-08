#![forbid(unsafe_code)]

use mail::sieve_validator::SieveValidator;
use mail::error::MailError;

#[test]
fn test_valid_sieve_script() {
    let validator = SieveValidator::new("user@example.com");
    let script = r#"
        require ["fileinto", "reject"];

        # Reject spam
        if header :contains "subject" "viagra" {
            discard;
            stop;
        }

        /* 
           Send important mail to a folder 
           and keep a copy.
        */
        elsif address :is "From" "boss@example.com" {
            fileinto "Important";
            keep;
        }

        else {
            redirect "other@example.com";
        }
    "#;

    assert!(validator.validate(script).is_ok());
}

#[test]
fn test_unbalanced_braces_extra_closing() {
    let validator = SieveValidator::new("user@example.com");
    let script = r#"
        if header :is "From" "a@b.com" {
            keep;
        }}
    "#;

    let result = validator.validate(script);
    assert!(matches!(result, Err(MailError::InvalidInput(ref msg)) if msg.contains("extra '}'")));
}

#[test]
fn test_unbalanced_braces_missing_closing() {
    let validator = SieveValidator::new("user@example.com");
    let script = r#"
        if header :is "From" "a@b.com" {
            keep;
    "#;

    let result = validator.validate(script);
    assert!(matches!(result, Err(MailError::InvalidInput(ref msg)) if msg.contains("missing '}'")));
}

#[test]
fn test_unterminated_quoted_string() {
    let validator = SieveValidator::new("user@example.com");
    let script = r#"
        if header :is "From" "a@b.com
    "#;

    let result = validator.validate(script);
    assert!(matches!(result, Err(MailError::InvalidInput(ref msg)) if msg.contains("Unterminated quoted string")));
}

#[test]
fn test_unterminated_bracket_comment() {
    let validator = SieveValidator::new("user@example.com");
    let script = r#"
        /* This comment never ends
        keep;
    "#;

    let result = validator.validate(script);
    assert!(matches!(result, Err(MailError::InvalidInput(ref msg)) if msg.contains("Unterminated bracket comment")));
}

#[test]
fn test_self_redirect() {
    let validator = SieveValidator::new("user@example.com");
    let script = r#"
        redirect "user@example.com";
    "#;

    let result = validator.validate(script);
    assert!(matches!(result, Err(MailError::InvalidInput(ref msg)) if msg.contains("Self-redirect detected")));
}

#[test]
fn test_case_insensitive_self_redirect() {
    let validator = SieveValidator::new("UsEr@exAmpLe.coM");
    let script = r#"
        redirect "user@example.com";
    "#;

    let result = validator.validate(script);
    assert!(matches!(result, Err(MailError::InvalidInput(ref msg)) if msg.contains("Self-redirect detected")));
}

#[test]
fn test_too_many_redirects() {
    let validator = SieveValidator::new("user@example.com");
    let script = r#"
        redirect "target1@example.com";
        redirect "target2@example.com";
        redirect "target3@example.com";
        redirect "target4@example.com";
        redirect "target5@example.com";
        redirect "target6@example.com";
    "#;

    let result = validator.validate(script);
    assert!(matches!(result, Err(MailError::InvalidInput(ref msg)) if msg.contains("Too many redirect targets")));
}

#[test]
fn test_multiline_text() {
    let validator = SieveValidator::new("user@example.com");
    let script = r#"
        reject text:
This is a multiline
rejection message.
.
;
    "#;
    assert!(validator.validate(script).is_ok());
}
