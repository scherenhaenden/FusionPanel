use administration::teams::invitation::{Invitation, InvitationError, Role};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn current_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[test]
fn test_invitation_creation_and_acceptance() {
    let tenant_id = Uuid::new_v4();
    let ttl = 3600; // 1 hour

    let mut inv = Invitation::new(tenant_id, Role::Admin, ttl);

    assert!(!inv.accepted);
    assert_eq!(inv.tenant_id, tenant_id);
    assert_eq!(inv.role, Role::Admin);

    let token = inv.token.clone();

    let result = inv.accept(&token, current_time());

    assert_eq!(result, Ok(Role::Admin));
    assert!(inv.accepted);
}

#[test]
fn test_invitation_already_accepted() {
    let tenant_id = Uuid::new_v4();
    let mut inv = Invitation::new(tenant_id, Role::Member, 3600);
    let token = inv.token.clone();

    // First accept works
    assert!(inv.accept(&token, current_time()).is_ok());

    // Second accept fails
    assert_eq!(
        inv.accept(&token, current_time()),
        Err(InvitationError::AlreadyAccepted)
    );
}

#[test]
fn test_invitation_expired() {
    let tenant_id = Uuid::new_v4();
    let mut inv = Invitation::new(tenant_id, Role::Viewer, 3600);
    let token = inv.token.clone();

    // Accept after expiry
    assert_eq!(
        inv.accept(&token, current_time() + 3601),
        Err(InvitationError::Expired)
    );
    assert!(!inv.accepted);
}

#[test]
fn test_invitation_invalid_token() {
    let tenant_id = Uuid::new_v4();
    let mut inv = Invitation::new(tenant_id, Role::Member, 3600);

    assert_eq!(
        inv.accept("invalid-token", current_time()),
        Err(InvitationError::InvalidToken)
    );
    assert!(!inv.accepted);
}

#[test]
fn test_timing_attack_resistance() {
    let tenant_id = Uuid::new_v4();
    let mut inv = Invitation::new(tenant_id, Role::Member, 3600);
    let token = inv.token.clone();

    // Create a token that matches partially
    let mut invalid_token = token.clone();
    invalid_token.replace_range(invalid_token.len() - 1.., "a");
    if invalid_token == token {
        invalid_token.replace_range(invalid_token.len() - 1.., "b");
    }

    assert_eq!(
        inv.accept(&invalid_token, current_time()),
        Err(InvitationError::InvalidToken)
    );
}
