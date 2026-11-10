#![forbid(unsafe_code)]

use administration::journald_tuner::{parse_size_to_bytes, JournaldPolicy};

#[test]
fn test_generate_conf() {
    let policy = JournaldPolicy::new("500M", "100M", "1month", 200);
    let conf = policy.generate_conf();
    assert_eq!(
        conf,
        "[Journal]\n\
         SystemMaxUse=500M\n\
         SystemKeepFree=100M\n\
         MaxRetentionSec=1month\n\
         RateLimitBurst=200\n"
    );
}

#[test]
fn test_vacuum_args() {
    let policy = JournaldPolicy::new("1G", "200M", "7d", 100);
    assert_eq!(policy.vacuum_size_arg(), "--vacuum-size=1G");
    assert_eq!(policy.vacuum_time_arg(), "--vacuum-time=7d");
}

#[test]
fn test_parse_size_to_bytes() {
    assert_eq!(parse_size_to_bytes("500M"), Some(524_288_000));
    assert_eq!(parse_size_to_bytes("1G"), Some(1_073_741_824));
    assert_eq!(parse_size_to_bytes("100K"), Some(102_400));
    assert_eq!(parse_size_to_bytes("2T"), Some(2_199_023_255_552));
    assert_eq!(parse_size_to_bytes("1.5G"), Some(1_610_612_736));
    assert_eq!(parse_size_to_bytes("invalid"), None);
    assert_eq!(parse_size_to_bytes(""), None);
    assert_eq!(parse_size_to_bytes("10"), Some(10));
}

#[test]
fn test_calculate_reclaimable_space() {
    let policy = JournaldPolicy::new("500M", "100M", "1month", 100);

    // 600M current size, max 500M -> 100M reclaimable
    assert_eq!(
        policy.calculate_reclaimable_space(629_145_600), // 600 * 1024 * 1024
        104_857_600 // 100 * 1024 * 1024
    );

    // 400M current size, max 500M -> 0 reclaimable
    assert_eq!(
        policy.calculate_reclaimable_space(419_430_400),
        0
    );

    // invalid format in max use
    let policy_invalid = JournaldPolicy::new("invalid", "100M", "1month", 100);
    assert_eq!(
        policy_invalid.calculate_reclaimable_space(1000),
        0 // parse_size_to_bytes returns u64::MAX, so saturating_sub is 0
    );
}
