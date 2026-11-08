#![forbid(unsafe_code)]

use administration::audit_chain::{
    AuditHashChain, AuditVerificationError, compute_hash, GENESIS_PREV_HASH,
};

#[test]
fn test_append_and_verify_chain() {
    let mut chain = AuditHashChain::new();

    let record1 = chain.append(
        1,
        "tenant1".to_string(),
        1000,
        "create".to_string(),
        "created user".to_string(),
    );

    assert_eq!(record1.prev_hash, GENESIS_PREV_HASH);
    let expected_hash1 = compute_hash(1, "tenant1", 1000, "create", "created user", GENESIS_PREV_HASH);
    assert_eq!(record1.hash, expected_hash1);

    let record2 = chain.append(
        2,
        "tenant1".to_string(),
        1001,
        "update".to_string(),
        "updated user".to_string(),
    );

    assert_eq!(record2.prev_hash, record1.hash);
    let expected_hash2 = compute_hash(2, "tenant1", 1001, "update", "updated user", &record1.hash);
    assert_eq!(record2.hash, expected_hash2);

    assert_eq!(chain.verify(), Ok(()));
}

#[test]
fn test_wrong_genesis_hash() {
    let mut chain = AuditHashChain::new();
    chain.append(
        1,
        "tenant1".to_string(),
        1000,
        "create".to_string(),
        "created user".to_string(),
    );

    chain.records[0].prev_hash = "wrong_genesis_hash".to_string();
    // Recompute the hash to make sure the modified record check doesn't fail first (though in this case it might fail on genesis check first)
    chain.records[0].hash = compute_hash(1, "tenant1", 1000, "create", "created user", "wrong_genesis_hash");

    assert_eq!(chain.verify(), Err(AuditVerificationError::InvalidGenesisHash(1)));
}

#[test]
fn test_tampered_entry() {
    let mut chain = AuditHashChain::new();
    chain.append(
        1,
        "tenant1".to_string(),
        1000,
        "create".to_string(),
        "created user".to_string(),
    );
    chain.append(
        2,
        "tenant1".to_string(),
        1001,
        "update".to_string(),
        "updated user".to_string(),
    );

    // Modify the action in the second record without updating its hash
    chain.records[1].action = "delete".to_string();

    assert_eq!(chain.verify(), Err(AuditVerificationError::ModifiedRecord(2)));
}

#[test]
fn test_broken_link() {
    let mut chain = AuditHashChain::new();
    chain.append(
        1,
        "tenant1".to_string(),
        1000,
        "create".to_string(),
        "created user".to_string(),
    );
    chain.append(
        2,
        "tenant1".to_string(),
        1001,
        "update".to_string(),
        "updated user".to_string(),
    );

    // Modify the prev_hash in the second record, simulating a broken link
    chain.records[1].prev_hash = "bad_link".to_string();
    // Update the hash to prevent `ModifiedRecord` error first
    let expected_hash = compute_hash(2, "tenant1", 1001, "update", "updated user", "bad_link");
    chain.records[1].hash = expected_hash;

    assert_eq!(chain.verify(), Err(AuditVerificationError::BrokenLink(2)));
}
