#![forbid(unsafe_code)]

use chrono::{DateTime, Datelike, Utc};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupArchive {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub destination: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub keep_daily: usize,
    pub keep_weekly: usize,
    pub keep_monthly: usize,
    pub keep_yearly: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionResult {
    pub retain: Vec<BackupArchive>,
    pub prune: Vec<BackupArchive>,
}

pub struct RetentionPolicyEngine;

impl RetentionPolicyEngine {
    pub fn evaluate(policy: &RetentionPolicy, mut backups: Vec<BackupArchive>) -> RetentionResult {
        // Sort backups by timestamp in descending order (newest first)
        backups.sort_by_key(|b| std::cmp::Reverse(b.timestamp));

        let mut retain = Vec::new();
        let mut prune = Vec::new();

        let mut daily_count = 0;
        let mut weekly_count = 0;
        let mut monthly_count = 0;
        let mut yearly_count = 0;

        let mut seen_days = HashSet::new();
        let mut seen_weeks = HashSet::new();
        let mut seen_months = HashSet::new();
        let mut seen_years = HashSet::new();

        for backup in backups {
            let t = backup.timestamp;

            let day_key = (t.year(), t.month(), t.day());
            let week_key = (t.iso_week().year(), t.iso_week().week());
            let month_key = (t.year(), t.month());
            let year_key = t.year();

            let mut kept = false;

            if daily_count < policy.keep_daily && !seen_days.contains(&day_key) {
                daily_count += 1;
                kept = true;
            }

            if weekly_count < policy.keep_weekly && !seen_weeks.contains(&week_key) {
                weekly_count += 1;
                kept = true;
            }

            if monthly_count < policy.keep_monthly && !seen_months.contains(&month_key) {
                monthly_count += 1;
                kept = true;
            }

            if yearly_count < policy.keep_yearly && !seen_years.contains(&year_key) {
                yearly_count += 1;
                kept = true;
            }

            seen_days.insert(day_key);
            seen_weeks.insert(week_key);
            seen_months.insert(month_key);
            seen_years.insert(year_key);

            if kept {
                retain.push(backup);
            } else {
                prune.push(backup);
            }
        }

        RetentionResult { retain, prune }
    }
}
