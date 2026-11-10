#![forbid(unsafe_code)]

use databases::{
    calculate_xid_age, evaluate_freeze_status, parse_frozen_xid, FreezeGuardConfig,
    FreezeGuardLevel,

};

#[test]
fn test_parse_frozen_xid() {
    assert_eq!(parse_frozen_xid("12345").unwrap(), 12345);
    assert_eq!(parse_frozen_xid("  67890  ").unwrap(), 67890);
    assert!(parse_frozen_xid("invalid").is_err());
    assert!(parse_frozen_xid("-1").is_err());
}

#[test]
fn test_calculate_xid_age_normal() {
    let current_xid: u32 = 1000;
    let frozen_xid: u32 = 500;
    assert_eq!(calculate_xid_age(current_xid, frozen_xid), 500);
}

#[test]
fn test_calculate_xid_age_wraparound() {
    let current_xid: u32 = 500; // Passed the wraparound
    let frozen_xid: u32 = (u32::MAX - 1000) + 1; // Near wraparound
    assert_eq!(calculate_xid_age(current_xid, frozen_xid), 1500);
}

#[test]
fn test_calculate_xid_age_exact_wraparound() {
    let current_xid: u32 = 0;
    let frozen_xid: u32 = u32::MAX;
    assert_eq!(calculate_xid_age(current_xid, frozen_xid), 1);
}

#[test]
fn test_evaluate_freeze_status_normal() {
    let config = FreezeGuardConfig::default();
    let status = evaluate_freeze_status("test_db", 2000, 1000, &config);

    assert_eq!(status.level, FreezeGuardLevel::Normal);
    assert_eq!(status.age, 1000);
    assert_eq!(status.remaining_to_autovacuum, config.autovacuum_freeze_max_age - 1000);
    assert_eq!(status.remaining_to_wraparound, config.wraparound_threshold - 1000);
}

#[test]
fn test_evaluate_freeze_status_warning_autovacuum() {
    let config = FreezeGuardConfig::default();
    let age = config.autovacuum_warning_threshold + 100;
    let current_xid = age + 1000;
    let frozen_xid = 1000;

    let status = evaluate_freeze_status("test_db", current_xid, frozen_xid, &config);

    assert_eq!(status.level, FreezeGuardLevel::WarningAutovacuum);
    assert_eq!(status.age, age);
}

#[test]
fn test_evaluate_freeze_status_critical_autovacuum() {
    let config = FreezeGuardConfig::default();
    let age = config.autovacuum_freeze_max_age + 100;
    let current_xid = age + 1000;
    let frozen_xid = 1000;

    let status = evaluate_freeze_status("test_db", current_xid, frozen_xid, &config);

    assert_eq!(status.level, FreezeGuardLevel::CriticalAutovacuum);
    assert_eq!(status.age, age);
}

#[test]
fn test_evaluate_freeze_status_warning_wraparound() {
    let config = FreezeGuardConfig::default();
    let age = config.wraparound_warning_threshold + 100;
    let current_xid = age + 1000;
    let frozen_xid = 1000;

    let status = evaluate_freeze_status("test_db", current_xid, frozen_xid, &config);

    assert_eq!(status.level, FreezeGuardLevel::WarningWraparound);
    assert_eq!(status.age, age);
}

#[test]
fn test_evaluate_freeze_status_emergency_wraparound() {
    let config = FreezeGuardConfig::default();
    let age = config.wraparound_threshold + 100;
    let current_xid = age + 1000;
    let frozen_xid = 1000;

    let status = evaluate_freeze_status("test_db", current_xid, frozen_xid, &config);

    assert_eq!(status.level, FreezeGuardLevel::EmergencyWraparound);
    assert_eq!(status.age, age);
    assert_eq!(status.remaining_to_wraparound, 0); // Testing saturating_sub
}

#[test]
fn test_evaluate_freeze_status_wraparound_math() {
    let config = FreezeGuardConfig::default();
    let frozen_xid = u32::MAX - 500;
    // We want the age to be just enough to trigger a WarningWraparound.
    let target_age = config.wraparound_warning_threshold + 100;
    // Current XID after wraparound
    let current_xid = frozen_xid.wrapping_add(target_age);

    let status = evaluate_freeze_status("test_db", current_xid, frozen_xid, &config);

    assert_eq!(status.level, FreezeGuardLevel::WarningWraparound);
    assert_eq!(status.age, target_age);
}

#[test]
fn test_saturating_sub_bounds() {
    let config = FreezeGuardConfig::default();
    let frozen_xid = 100;
    let current_xid = frozen_xid + config.wraparound_threshold + 1000; // Way over threshold

    let status = evaluate_freeze_status("test_db", current_xid, frozen_xid, &config);

    assert_eq!(status.remaining_to_autovacuum, 0);
    assert_eq!(status.remaining_to_wraparound, 0);
    assert_eq!(status.level, FreezeGuardLevel::EmergencyWraparound);
}
