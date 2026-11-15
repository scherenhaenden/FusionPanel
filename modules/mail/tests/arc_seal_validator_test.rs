#![forbid(unsafe_code)]

use mail::arc_seal_validator::{ArcChainValidator, ArcError, ArcValidationResult};

#[test]
fn test_single_hop_valid() {
    let validator = ArcChainValidator::new();
    let headers = vec![
        ("ARC-Authentication-Results", "i=1; mx.google.com; dkim=pass"),
        ("ARC-Message-Signature", "i=1; a=rsa-sha256; c=relaxed/relaxed; d=example.com; s=selector; h=from:to:subject:date; bh=...; b=..."),
        ("ARC-Seal", "i=1; a=rsa-sha256; t=12345; cv=none; d=example.com; s=selector; b=..."),
    ];

    assert_eq!(validator.validate(&headers), Ok(ArcValidationResult::Pass));
}

#[test]
fn test_multi_hop_valid() {
    let validator = ArcChainValidator::new();
    let headers = vec![
        // Hop 2
        ("ARC-Authentication-Results", "i=2; mx.google.com; dkim=pass"),
        ("ARC-Message-Signature", "i=2; a=rsa-sha256; c=relaxed/relaxed; d=example.org; s=selector; h=from:to:subject:date; bh=...; b=..."),
        ("ARC-Seal", "i=2; a=rsa-sha256; t=12346; cv=pass; d=example.org; s=selector; b=..."),
        // Hop 1
        ("ARC-Authentication-Results", "i=1; mx.google.com; dkim=pass"),
        ("ARC-Message-Signature", "i=1; a=rsa-sha256; c=relaxed/relaxed; d=example.com; s=selector; h=from:to:subject:date; bh=...; b=..."),
        ("ARC-Seal", "i=1; a=rsa-sha256; t=12345; cv=none; d=example.com; s=selector; b=..."),
    ];

    assert_eq!(validator.validate(&headers), Ok(ArcValidationResult::Pass));
}

#[test]
fn test_invalid_sequence_missing_aar() {
    let validator = ArcChainValidator::new();
    let headers = vec![
        ("ARC-Message-Signature", "i=1; a=rsa-sha256; c=relaxed/relaxed; d=example.com; s=selector; h=from:to:subject:date; bh=...; b=..."),
        ("ARC-Seal", "i=1; a=rsa-sha256; t=12345; cv=none; d=example.com; s=selector; b=..."),
    ];

    assert_eq!(validator.validate(&headers), Ok(ArcValidationResult::InvalidSequence));
}

#[test]
fn test_invalid_sequence_non_consecutive() {
    let validator = ArcChainValidator::new();
    let headers = vec![
        // Hop 3
        ("ARC-Authentication-Results", "i=3; mx.google.com; dkim=pass"),
        ("ARC-Message-Signature", "i=3; a=rsa-sha256; c=relaxed/relaxed; d=example.org; s=selector; h=from:to:subject:date; bh=...; b=..."),
        ("ARC-Seal", "i=3; a=rsa-sha256; t=12346; cv=pass; d=example.org; s=selector; b=..."),
        // Hop 1
        ("ARC-Authentication-Results", "i=1; mx.google.com; dkim=pass"),
        ("ARC-Message-Signature", "i=1; a=rsa-sha256; c=relaxed/relaxed; d=example.com; s=selector; h=from:to:subject:date; bh=...; b=..."),
        ("ARC-Seal", "i=1; a=rsa-sha256; t=12345; cv=none; d=example.com; s=selector; b=..."),
    ];

    assert_eq!(validator.validate(&headers), Ok(ArcValidationResult::InvalidSequence));
}

#[test]
fn test_broken_cv_first_hop() {
    let validator = ArcChainValidator::new();
    let headers = vec![
        ("ARC-Authentication-Results", "i=1; mx.google.com; dkim=pass"),
        ("ARC-Message-Signature", "i=1; a=rsa-sha256; c=relaxed/relaxed; d=example.com; s=selector; h=from:to:subject:date; bh=...; b=..."),
        ("ARC-Seal", "i=1; a=rsa-sha256; t=12345; cv=pass; d=example.com; s=selector; b=..."), // Should be cv=none
    ];

    assert_eq!(validator.validate(&headers), Ok(ArcValidationResult::Fail));
}

#[test]
fn test_broken_cv_second_hop() {
    let validator = ArcChainValidator::new();
    let headers = vec![
        // Hop 2
        ("ARC-Authentication-Results", "i=2; mx.google.com; dkim=pass"),
        ("ARC-Message-Signature", "i=2; a=rsa-sha256; c=relaxed/relaxed; d=example.org; s=selector; h=from:to:subject:date; bh=...; b=..."),
        ("ARC-Seal", "i=2; a=rsa-sha256; t=12346; cv=fail; d=example.org; s=selector; b=..."), // Should be cv=pass
        // Hop 1
        ("ARC-Authentication-Results", "i=1; mx.google.com; dkim=pass"),
        ("ARC-Message-Signature", "i=1; a=rsa-sha256; c=relaxed/relaxed; d=example.com; s=selector; h=from:to:subject:date; bh=...; b=..."),
        ("ARC-Seal", "i=1; a=rsa-sha256; t=12345; cv=none; d=example.com; s=selector; b=..."),
    ];

    assert_eq!(validator.validate(&headers), Ok(ArcValidationResult::Fail));
}

#[test]
fn test_missing_instance_number() {
    let validator = ArcChainValidator::new();
    let headers = vec![
        ("ARC-Authentication-Results", "mx.google.com; dkim=pass"),
    ];

    assert_eq!(validator.validate(&headers), Err(ArcError::MissingInstance));
}

#[test]
fn test_invalid_instance_number() {
    let validator = ArcChainValidator::new();
    let headers = vec![
        ("ARC-Authentication-Results", "i=abc; mx.google.com; dkim=pass"),
    ];

    assert_eq!(validator.validate(&headers), Err(ArcError::InvalidInstance));
}

#[test]
fn test_zero_instance_number() {
    let validator = ArcChainValidator::new();
    let headers = vec![
        ("ARC-Authentication-Results", "i=0; mx.google.com; dkim=pass"),
    ];

    assert_eq!(validator.validate(&headers), Err(ArcError::InvalidInstance));
}

#[test]
fn test_no_arc_headers() {
    let validator = ArcChainValidator::new();
    let headers = vec![
        ("From", "user@example.com"),
    ];

    assert_eq!(validator.validate(&headers), Ok(ArcValidationResult::Pass));
}
