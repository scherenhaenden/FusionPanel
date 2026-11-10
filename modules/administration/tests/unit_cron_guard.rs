#![forbid(unsafe_code)]

use administration::cron_guard::{CronSchedule, calculate_jitter};
use chrono::{TimeZone, Utc};

#[test]
fn test_parse_valid_expressions() {
    let sched = CronSchedule::parse("0 12 * * *").unwrap();
    assert_eq!(sched.minutes.len(), 1);
    assert!(sched.minutes.contains(&0));
    assert_eq!(sched.hours.len(), 1);
    assert!(sched.hours.contains(&12));
    assert_eq!(sched.days_of_month.len(), 31);
    assert_eq!(sched.months.len(), 12);
    assert_eq!(sched.days_of_week.len(), 8); // 0-7
    assert!(!sched.dom_restricted);
    assert!(!sched.dow_restricted);
}

#[test]
fn test_parse_ranges_and_lists() {
    let sched = CronSchedule::parse("0-15,30-45 * 1,15 * 1-5").unwrap();
    assert_eq!(sched.minutes.len(), 32);
    assert!(sched.minutes.contains(&0));
    assert!(sched.minutes.contains(&15));
    assert!(sched.minutes.contains(&30));
    assert!(sched.minutes.contains(&45));
    assert!(!sched.minutes.contains(&20));

    assert_eq!(sched.days_of_month.len(), 2);
    assert!(sched.days_of_month.contains(&1));
    assert!(sched.days_of_month.contains(&15));

    assert_eq!(sched.days_of_week.len(), 5);
    assert!(sched.days_of_week.contains(&1));
    assert!(sched.days_of_week.contains(&5));

    assert!(sched.dom_restricted);
    assert!(sched.dow_restricted);
}

#[test]
fn test_parse_steps() {
    let sched = CronSchedule::parse("*/15 */6 * * *").unwrap();
    assert_eq!(sched.minutes.len(), 4);
    assert!(sched.minutes.contains(&0));
    assert!(sched.minutes.contains(&15));
    assert!(sched.minutes.contains(&30));
    assert!(sched.minutes.contains(&45));

    assert_eq!(sched.hours.len(), 4);
    assert!(sched.hours.contains(&0));
    assert!(sched.hours.contains(&6));
    assert!(sched.hours.contains(&12));
    assert!(sched.hours.contains(&18));
}

#[test]
fn test_jitter_calculation_deterministic() {
    let j1 = calculate_jitter("tenant-1-job-1");
    let j2 = calculate_jitter("tenant-1-job-1");
    assert_eq!(j1, j2);
    assert!(j1 < 60);

    let j3 = calculate_jitter("tenant-1-job-2");
    assert!(j3 < 60);
}

#[test]
fn test_parse_with_jitter() {
    let seed = "some-tenant-job";
    let jitter = calculate_jitter(seed);

    let sched = CronSchedule::parse_with_jitter("0 12 * * *", seed).unwrap();
    assert_eq!(sched.minutes.len(), 1);
    assert!(sched.minutes.contains(&jitter));

    let sched2 = CronSchedule::parse_with_jitter("*/15 12 * * *", seed).unwrap();
    assert_eq!(sched2.minutes.len(), 4);
    assert!(sched2.minutes.contains(&(jitter % 60)));
    assert!(sched2.minutes.contains(&((15 + jitter) % 60)));
    assert!(sched2.minutes.contains(&((30 + jitter) % 60)));
    assert!(sched2.minutes.contains(&((45 + jitter) % 60)));
}

#[test]
fn test_next_execution() {
    let sched = CronSchedule::parse("30 14 15 * *").unwrap();
    let now = Utc.with_ymd_and_hms(2023, 5, 1, 0, 0, 0).unwrap();
    let next = sched.next_execution(now).unwrap();
    assert_eq!(next, Utc.with_ymd_and_hms(2023, 5, 15, 14, 30, 0).unwrap());
}

#[test]
fn test_next_execution_dow_dom_or_behavior() {
    // If both DOM and DOW are specified, they form an OR condition in standard cron
    let sched = CronSchedule::parse("0 12 1 * 5").unwrap();
    // 1st of month OR a Friday
    let now = Utc.with_ymd_and_hms(2023, 11, 2, 0, 0, 0).unwrap(); // Nov 2, 2023 is Thursday
    let next = sched.next_execution(now).unwrap();
    // Next execution should be Friday Nov 3
    assert_eq!(next, Utc.with_ymd_and_hms(2023, 11, 3, 12, 0, 0).unwrap());

    // Now testing the DOM part
    let now2 = Utc.with_ymd_and_hms(2023, 11, 30, 13, 0, 0).unwrap();
    let next2 = sched.next_execution(now2).unwrap();
    // Next execution should be Dec 1 (Friday and 1st)
    assert_eq!(next2, Utc.with_ymd_and_hms(2023, 12, 1, 12, 0, 0).unwrap());
}
