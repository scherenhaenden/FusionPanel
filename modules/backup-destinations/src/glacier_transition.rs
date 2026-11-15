#![forbid(unsafe_code)]

use std::cmp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum S3StorageClass {
    Standard,
    StandardIA,
    GlacierFlexible,
    DeepArchive,
}

impl S3StorageClass {
    pub fn min_retention_days(&self) -> u32 {
        match self {
            S3StorageClass::Standard => 0,
            S3StorageClass::StandardIA => 30,
            S3StorageClass::GlacierFlexible => 90,
            S3StorageClass::DeepArchive => 180,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionRule {
    pub target_class: S3StorageClass,
    pub days_after_creation: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveTransition {
    pub target_class: S3StorageClass,
    pub effective_days: u32,
}

#[derive(Debug, Clone)]
pub struct LifecycleScheduler {
    rules: Vec<TransitionRule>,
}

impl LifecycleScheduler {
    pub fn new(rules: Vec<TransitionRule>) -> Self {
        Self { rules }
    }

    pub fn calculate_effective_schedule(&self) -> Vec<EffectiveTransition> {
        let mut sorted_rules = self.rules.clone();
        // Sort by days_after_creation, then by storage class
        sorted_rules.sort_by_key(|r| (r.days_after_creation, r.target_class));

        let mut effective_schedule: Vec<EffectiveTransition> = Vec::new();
        let mut current_class = S3StorageClass::Standard;
        let mut class_start_day = 0;

        for rule in sorted_rules {
            if rule.target_class <= current_class {
                // Ignore backwards or same-class transitions
                continue;
            }

            let min_days_for_current_class = current_class.min_retention_days();
            let earliest_possible_transition = class_start_day + min_days_for_current_class;

            let effective_days = cmp::max(rule.days_after_creation, earliest_possible_transition);

            effective_schedule.push(EffectiveTransition {
                target_class: rule.target_class,
                effective_days,
            });

            current_class = rule.target_class;
            class_start_day = effective_days;
        }

        effective_schedule
    }
}
