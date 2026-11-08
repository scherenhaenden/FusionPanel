#![forbid(unsafe_code)]

use monitoring::governor::SystemGovernor;

#[test]
fn test_governor_instantiation() {
    let gov_res = SystemGovernor::new(1.0, 1.0);
    assert!(gov_res.is_ok());
}

#[test]
fn test_compute_load_score() {
    let mut gov = SystemGovernor::new(0.5, 0.5).unwrap();
    let score = gov.compute_load_score().unwrap();
    // In a test environment, load is non-negative and likely > 0 if there's any memory usage
    assert!(score >= 0.0);
}

#[test]
fn test_is_near_oom() {
    let mut gov = SystemGovernor::new(1.0, 1.0).unwrap();
    // 101% is impossible to reach, so it should not be near OOM
    let near_oom = gov.is_near_oom(101.0).unwrap();
    assert!(!near_oom);

    // 0% should always be reached (since some memory is used)
    let near_oom = gov.is_near_oom(0.0).unwrap();
    assert!(near_oom);
}

#[test]
fn test_identify_throttle_candidates() {
    let mut gov = SystemGovernor::new(1.0, 1.0).unwrap();

    // An arbitrarily large memory amount that no process should consume
    // 100 TB (or more than physical limit)
    let large_memory = u64::MAX;
    let candidates = gov.identify_throttle_candidates(large_memory).unwrap();
    assert_eq!(candidates.len(), 0);

    // 0 bytes threshold should return at least some processes in a typical system
    let candidates_0 = gov.identify_throttle_candidates(0).unwrap();
    assert!(candidates_0.len() > 0);
}
