#![forbid(unsafe_code)]

#[path = "../src/alias.rs"]
mod alias;

use alias::{AliasError, AliasRouter};

#[test]
fn test_valid_alias() {
    let mut router = AliasRouter::new();
    assert_eq!(
        router.add_alias(
            "info@example.com",
            vec!["alice@example.com", "bob@example.com"]
        ),
        Ok(())
    );
}

#[test]
fn test_invalid_email() {
    let mut router = AliasRouter::new();
    assert_eq!(
        router.add_alias("infoexample.com", vec!["alice@example.com"]),
        Err(AliasError::InvalidAddress("infoexample.com".to_string()))
    );
}

#[test]
fn test_cyclic_loop_detection_simple() {
    let mut router = AliasRouter::new();
    assert_eq!(
        router.add_alias("a@example.com", vec!["b@example.com"]),
        Ok(())
    );
    assert_eq!(
        router.add_alias("b@example.com", vec!["c@example.com"]),
        Ok(())
    );

    let err = router
        .add_alias("c@example.com", vec!["a@example.com"])
        .unwrap_err();
    match err {
        AliasError::CyclicLoop(s) => {
            assert!(s == "a@example.com" || s == "b@example.com" || s == "c@example.com");
        }
        _ => panic!("Expected CyclicLoop error"),
    }
}

#[test]
fn test_cyclic_loop_detection_self() {
    let mut router = AliasRouter::new();
    assert_eq!(
        router.add_alias("a@example.com", vec!["a@example.com"]),
        Err(AliasError::CyclicLoop("a@example.com".to_string()))
    );
}

#[test]
fn test_cyclic_loop_with_catch_all() {
    let mut router = AliasRouter::new();
    assert_eq!(
        router.add_catch_all("example.com", vec!["admin@example.com"]),
        Ok(())
    );
    let err = router
        .add_alias("admin@example.com", vec!["unknown@example.com"])
        .unwrap_err();
    match err {
        AliasError::CyclicLoop(s) => {
            assert!(s == "admin@example.com" || s == "unknown@example.com");
        }
        _ => panic!("Expected CyclicLoop error"),
    }
}

#[test]
fn test_catch_all_domain_mismatch() {
    let mut router = AliasRouter::new();
    assert_eq!(
        router.add_catch_all("example.com", vec!["admin@other.com"]),
        Err(AliasError::DomainMismatch(
            "example.com".to_string(),
            "other.com".to_string()
        ))
    );
}

#[test]
fn test_catch_all_valid() {
    let mut router = AliasRouter::new();
    assert_eq!(
        router.add_catch_all("example.com", vec!["admin@example.com"]),
        Ok(())
    );
}

#[test]
fn test_render_table() {
    let mut router = AliasRouter::new();
    router
        .add_alias(
            "info@example.com",
            vec!["alice@example.com", "bob@example.com"],
        )
        .unwrap();
    router
        .add_alias("sales@example.com", vec!["charlie@example.com"])
        .unwrap();
    router
        .add_catch_all("example.com", vec!["admin@example.com"])
        .unwrap();
    router
        .add_catch_all("test.com", vec!["root@test.com"])
        .unwrap();

    let table = router.render_table();
    let expected = "\
info@example.com: alice@example.com, bob@example.com
sales@example.com: charlie@example.com
@*: example.com -> admin@example.com
@*: test.com -> root@test.com";
    assert_eq!(table, expected);
}
