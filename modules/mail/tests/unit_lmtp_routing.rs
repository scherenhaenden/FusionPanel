#![forbid(unsafe_code)]

use mail::lmtp_routing::{DeliveryTarget, LmtpRouter, ResolvedRoute, RoutingError};

#[test]
fn test_resolve_mailbox() {
    let mut router = LmtpRouter::new();
    router.add_mailbox(
        "user@example.com".to_string(),
        DeliveryTarget::UnixSocket("/var/run/dovecot/lmtp".to_string()),
    );

    let result = router.resolve("user@example.com").unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        ResolvedRoute {
            recipient: "user@example.com".to_string(),
            target: DeliveryTarget::UnixSocket("/var/run/dovecot/lmtp".to_string()),
        }
    );
}

#[test]
fn test_resolve_not_found() {
    let router = LmtpRouter::new();
    let err = router.resolve("missing@example.com").unwrap_err();
    assert_eq!(
        err,
        RoutingError::RecipientNotFound("missing@example.com".to_string())
    );
}

#[test]
fn test_resolve_alias_single() {
    let mut router = LmtpRouter::new();
    router.add_mailbox(
        "user@example.com".to_string(),
        DeliveryTarget::Lmtp {
            host: "127.0.0.1".to_string(),
            port: 24,
        },
    );
    router.add_alias(
        "alias@example.com".to_string(),
        vec!["user@example.com".to_string()],
    );

    let result = router.resolve("alias@example.com").unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        ResolvedRoute {
            recipient: "user@example.com".to_string(),
            target: DeliveryTarget::Lmtp {
                host: "127.0.0.1".to_string(),
                port: 24,
            },
        }
    );
}

#[test]
fn test_resolve_alias_multiple() {
    let mut router = LmtpRouter::new();
    router.add_mailbox(
        "user1@example.com".to_string(),
        DeliveryTarget::UnixSocket("/sock1".to_string()),
    );
    router.add_mailbox(
        "user2@example.com".to_string(),
        DeliveryTarget::UnixSocket("/sock2".to_string()),
    );
    router.add_alias(
        "list@example.com".to_string(),
        vec!["user1@example.com".to_string(), "user2@example.com".to_string()],
    );

    let mut result = router.resolve("list@example.com").unwrap();
    result.sort_by(|a, b| a.recipient.cmp(&b.recipient));
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].recipient, "user1@example.com");
    assert_eq!(result[1].recipient, "user2@example.com");
}

#[test]
fn test_resolve_diamond_dependency() {
    // alias -> a1, a2
    // a1 -> target
    // a2 -> target
    // target should only appear once.
    let mut router = LmtpRouter::new();
    router.add_mailbox(
        "target@example.com".to_string(),
        DeliveryTarget::UnixSocket("/target".to_string()),
    );
    router.add_alias(
        "a1@example.com".to_string(),
        vec!["target@example.com".to_string()],
    );
    router.add_alias(
        "a2@example.com".to_string(),
        vec!["target@example.com".to_string()],
    );
    router.add_alias(
        "alias@example.com".to_string(),
        vec!["a1@example.com".to_string(), "a2@example.com".to_string()],
    );

    let result = router.resolve("alias@example.com").unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].recipient, "target@example.com");
}

#[test]
fn test_resolve_alias_loop_self() {
    let mut router = LmtpRouter::new();
    router.add_alias(
        "loop@example.com".to_string(),
        vec!["loop@example.com".to_string()],
    );

    let err = router.resolve("loop@example.com").unwrap_err();
    assert_eq!(
        err,
        RoutingError::AliasLoopDetected("loop@example.com".to_string())
    );
}

#[test]
fn test_resolve_alias_loop_deep() {
    let mut router = LmtpRouter::new();
    router.add_alias(
        "a@example.com".to_string(),
        vec!["b@example.com".to_string()],
    );
    router.add_alias(
        "b@example.com".to_string(),
        vec!["c@example.com".to_string()],
    );
    router.add_alias(
        "c@example.com".to_string(),
        vec!["a@example.com".to_string()],
    );

    let err = router.resolve("a@example.com").unwrap_err();
    assert!(matches!(err, RoutingError::AliasLoopDetected(_)));
}
