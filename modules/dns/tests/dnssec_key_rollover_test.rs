#![forbid(unsafe_code)]

use dns::dnssec_key_rollover::{
    DnssecRolloverPlanner, KeyPhase, KeyType, RolloverAction, RolloverError, RolloverMethod,
    ZoneTimingParameters,
};
use std::time::Duration;

fn create_test_timing() -> ZoneTimingParameters {
    ZoneTimingParameters {
        zone_ttl: Duration::from_secs(3600), // 1 hour
        propagation_delay: Duration::from_secs(300), // 5 mins
        parent_ds_ttl: Duration::from_secs(86400), // 1 day
        parent_propagation_delay: Duration::from_secs(3600), // 1 hour
        max_zone_ttl: Duration::from_secs(7200), // 2 hours
    }
}

#[test]
fn test_valid_transitions() {
    assert!(DnssecRolloverPlanner::validate_transition(KeyPhase::Generated, KeyPhase::Published).is_ok());
    assert!(DnssecRolloverPlanner::validate_transition(KeyPhase::Published, KeyPhase::Active).is_ok());
    assert!(DnssecRolloverPlanner::validate_transition(KeyPhase::Active, KeyPhase::Retired).is_ok());
    assert!(DnssecRolloverPlanner::validate_transition(KeyPhase::Retired, KeyPhase::Dead).is_ok());
}

#[test]
fn test_invalid_transitions() {
    let err = DnssecRolloverPlanner::validate_transition(KeyPhase::Generated, KeyPhase::Active).unwrap_err();
    match err {
        RolloverError::InvalidTransition { from, to } => {
            assert_eq!(from, KeyPhase::Generated);
            assert_eq!(to, KeyPhase::Active);
        }
        _ => panic!("Expected InvalidTransition"),
    }
}

#[test]
fn test_unsupported_method() {
    let planner = DnssecRolloverPlanner::new(create_test_timing());

    // ZSK DoubleDS is not supported (usually only for KSK)
    let err = planner.generate_schedule(KeyType::ZSK, RolloverMethod::DoubleDS).unwrap_err();
    match err {
        RolloverError::UnsupportedMethod(m, kt) => {
            assert_eq!(m, RolloverMethod::DoubleDS);
            assert_eq!(kt, KeyType::ZSK);
        }
        _ => panic!("Expected UnsupportedMethod"),
    }

    // KSK PrePublish is not supported
    let err = planner.generate_schedule(KeyType::KSK, RolloverMethod::PrePublish).unwrap_err();
    match err {
        RolloverError::UnsupportedMethod(m, kt) => {
            assert_eq!(m, RolloverMethod::PrePublish);
            assert_eq!(kt, KeyType::KSK);
        }
        _ => panic!("Expected UnsupportedMethod"),
    }
}

#[test]
fn test_zsk_pre_publish_schedule() {
    let timing = create_test_timing();
    let planner = DnssecRolloverPlanner::new(timing.clone());

    let schedule = planner.generate_schedule(KeyType::ZSK, RolloverMethod::PrePublish).unwrap();

    assert_eq!(schedule.len(), 4);

    assert_eq!(schedule[0].action, RolloverAction::PublishNewKey);
    assert_eq!(schedule[0].wait_before, Duration::ZERO);

    assert_eq!(schedule[1].action, RolloverAction::ActivateNewKey);
    assert_eq!(schedule[1].wait_before, timing.propagation_delay + timing.zone_ttl);

    assert_eq!(schedule[2].action, RolloverAction::RetireOldKey);
    assert_eq!(schedule[2].wait_before, Duration::ZERO);

    assert_eq!(schedule[3].action, RolloverAction::RemoveOldKey);
    assert_eq!(schedule[3].wait_before, timing.propagation_delay + timing.max_zone_ttl);
}

#[test]
fn test_ksk_double_ds_schedule() {
    let timing = create_test_timing();
    let planner = DnssecRolloverPlanner::new(timing.clone());

    let schedule = planner.generate_schedule(KeyType::KSK, RolloverMethod::DoubleDS).unwrap();

    assert_eq!(schedule.len(), 6);

    assert_eq!(schedule[0].action, RolloverAction::PublishNewKey);
    assert_eq!(schedule[0].wait_before, Duration::ZERO);

    assert_eq!(schedule[1].action, RolloverAction::PublishNewDs);
    assert_eq!(schedule[1].wait_before, timing.propagation_delay + timing.zone_ttl);

    assert_eq!(schedule[2].action, RolloverAction::ActivateNewKey);
    assert_eq!(schedule[2].wait_before, timing.parent_propagation_delay + timing.parent_ds_ttl);

    assert_eq!(schedule[3].action, RolloverAction::RetireOldKey);
    assert_eq!(schedule[3].wait_before, Duration::ZERO);

    assert_eq!(schedule[4].action, RolloverAction::RemoveOldDs);
    assert_eq!(schedule[4].wait_before, Duration::ZERO);

    assert_eq!(schedule[5].action, RolloverAction::RemoveOldKey);
    assert_eq!(schedule[5].wait_before, timing.parent_propagation_delay + timing.parent_ds_ttl);
}
