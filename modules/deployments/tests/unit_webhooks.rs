use deployments::webhooks::{github, gitlab};

#[test]
fn test_github_signature_verification() {
    let secret = "my_super_secret";
    let payload = b"{\"ref\":\"refs/heads/main\"}";

    // Calculated using HMAC-SHA256
    let valid_signature = "sha256=190fddb1da7f14f3c307772a08c5f895b6dee6ca601d12248ffb4fb2c9812e61";

    assert!(github::verify_signature(secret, valid_signature, payload).is_ok());

    let invalid_signature =
        "sha256=1234d49405d4f3b79ce8ec48074983b63fa282d1c990ccde65d9fc378d386e88";
    assert!(github::verify_signature(secret, invalid_signature, payload).is_err());

    let invalid_format = "sha1=190fddb1da7f14f3c307772a08c5f895b6dee6ca601d12248ffb4fb2c9812e61";
    assert!(github::verify_signature(secret, invalid_format, payload).is_err());
}

#[test]
fn test_gitlab_token_verification() {
    let secret = "my_gitlab_secret";

    assert!(gitlab::verify_token(secret, "my_gitlab_secret").is_ok());
    assert!(gitlab::verify_token(secret, "wrong_secret").is_err());
}

#[test]
fn test_github_event_parsing() {
    let push_payload = b"{\"ref\":\"refs/heads/main\",\"after\":\"abcdef123456\"}";
    let event = github::parse_event("push", push_payload).unwrap();
    if let github::GithubEvent::Push { r#ref, commit_sha } = event {
        assert_eq!(r#ref, "refs/heads/main");
        assert_eq!(commit_sha, "abcdef123456");
    } else {
        panic!("Expected Push event");
    }

    let tag_payload = b"{\"ref\":\"refs/tags/v1.0.0\"}";
    let event = github::parse_event("push", tag_payload).unwrap();
    if let github::GithubEvent::Tag { r#ref } = event {
        assert_eq!(r#ref, "refs/tags/v1.0.0");
    } else {
        panic!("Expected Tag event");
    }
}

#[test]
fn test_gitlab_event_parsing() {
    let push_payload = b"{\"ref\":\"refs/heads/main\",\"after\":\"abcdef123456\"}";
    let event = gitlab::parse_event("Push Hook", push_payload).unwrap();
    if let gitlab::GitlabEvent::Push { r#ref, commit_sha } = event {
        assert_eq!(r#ref, "refs/heads/main");
        assert_eq!(commit_sha, "abcdef123456");
    } else {
        panic!("Expected Push event");
    }

    let tag_payload = b"{\"ref\":\"refs/tags/v1.0.0\"}";
    let event = gitlab::parse_event("Tag Push Hook", tag_payload).unwrap();
    if let gitlab::GitlabEvent::Tag { r#ref } = event {
        assert_eq!(r#ref, "refs/tags/v1.0.0");
    } else {
        panic!("Expected Tag event");
    }
}
