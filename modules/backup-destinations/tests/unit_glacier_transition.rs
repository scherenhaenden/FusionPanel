#![forbid(unsafe_code)]

use backup_destinations::glacier_transition::{
    LifecycleScheduler, S3StorageClass, TransitionRule,
};

#[test]
fn test_standard_to_ia() {
    let rules = vec![TransitionRule {
        target_class: S3StorageClass::StandardIA,
        days_after_creation: 30,
    }];
    let scheduler = LifecycleScheduler::new(rules);
    let schedule = scheduler.calculate_effective_schedule();

    assert_eq!(schedule.len(), 1);
    assert_eq!(schedule[0].target_class, S3StorageClass::StandardIA);
    assert_eq!(schedule[0].effective_days, 30);
}

#[test]
fn test_minimum_retention_guard() {
    // If user tries to transition to Glacier 10 days after IA,
    // it should be pushed to 30 days after IA transition.
    let rules = vec![
        TransitionRule {
            target_class: S3StorageClass::StandardIA,
            days_after_creation: 30,
        },
        TransitionRule {
            target_class: S3StorageClass::GlacierFlexible,
            days_after_creation: 40, // 30 (IA start) + 10 = 40. But IA requires 30 days min.
        },
    ];
    let scheduler = LifecycleScheduler::new(rules);
    let schedule = scheduler.calculate_effective_schedule();

    assert_eq!(schedule.len(), 2);
    assert_eq!(schedule[0].target_class, S3StorageClass::StandardIA);
    assert_eq!(schedule[0].effective_days, 30);

    assert_eq!(schedule[1].target_class, S3StorageClass::GlacierFlexible);
    // 30 days (IA effective) + 30 days (IA min retention) = 60
    assert_eq!(schedule[1].effective_days, 60);
}

#[test]
fn test_full_lifecycle() {
    let rules = vec![
        TransitionRule {
            target_class: S3StorageClass::StandardIA,
            days_after_creation: 10,
        },
        TransitionRule {
            target_class: S3StorageClass::GlacierFlexible,
            days_after_creation: 30,
        },
        TransitionRule {
            target_class: S3StorageClass::DeepArchive,
            days_after_creation: 90,
        },
    ];
    let scheduler = LifecycleScheduler::new(rules);
    let schedule = scheduler.calculate_effective_schedule();

    assert_eq!(schedule.len(), 3);

    assert_eq!(schedule[0].target_class, S3StorageClass::StandardIA);
    assert_eq!(schedule[0].effective_days, 10); // Standard has 0 min retention

    assert_eq!(schedule[1].target_class, S3StorageClass::GlacierFlexible);
    assert_eq!(schedule[1].effective_days, 40); // 10 (IA start) + 30 (IA min) = 40

    assert_eq!(schedule[2].target_class, S3StorageClass::DeepArchive);
    assert_eq!(schedule[2].effective_days, 130); // 40 (Glacier start) + 90 (Glacier min) = 130
}

#[test]
fn test_ignore_backward_transitions() {
    let rules = vec![
        TransitionRule {
            target_class: S3StorageClass::GlacierFlexible,
            days_after_creation: 90,
        },
        TransitionRule {
            target_class: S3StorageClass::StandardIA,
            days_after_creation: 120, // should be ignored
        },
    ];
    let scheduler = LifecycleScheduler::new(rules);
    let schedule = scheduler.calculate_effective_schedule();

    assert_eq!(schedule.len(), 1);
    assert_eq!(schedule[0].target_class, S3StorageClass::GlacierFlexible);
    assert_eq!(schedule[0].effective_days, 90);
}
