#![forbid(unsafe_code)]

use chrono::{DateTime, Datelike, Utc};
use std::collections::{HashSet};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum GfsError {
    #[error("Retention configuration error: {0}")]
    ConfigError(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GfsRetentionPolicy {
    pub keep_daily: usize,
    pub keep_weekly: usize,
    pub keep_monthly: usize,
    pub keep_yearly: usize,
}

impl GfsRetentionPolicy {
    pub fn new(daily: usize, weekly: usize, monthly: usize, yearly: usize) -> Result<Self, GfsError> {
        Ok(Self {
            keep_daily: daily,
            keep_weekly: weekly,
            keep_monthly: monthly,
            keep_yearly: yearly,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GfsPlanResult<T> {
    pub retained: Vec<T>,
    pub pruned: Vec<T>,
}

pub struct GfsRetentionPlanner;

impl GfsRetentionPlanner {
    /// Planns the retention based on a GFS policy.
    /// `snapshots` is a list of items and their corresponding timestamps.
    /// The algorithm deterministically categorizes snapshots into retained tiers and marks others for pruning.
    /// It NEVER prunes the most recent snapshot regardless of retention rules.
    pub fn plan_retention<T: Clone>(
        snapshots: &[(T, DateTime<Utc>)],
        policy: &GfsRetentionPolicy,
    ) -> GfsPlanResult<T> {
        if snapshots.is_empty() {
            return GfsPlanResult {
                retained: Vec::new(),
                pruned: Vec::new(),
            };
        }

        // Sort snapshots from newest to oldest
        let mut sorted_snapshots: Vec<(T, DateTime<Utc>)> = snapshots.to_vec();
        sorted_snapshots.sort_by(|a, b| b.1.cmp(&a.1));

        let mut retained_indices: HashSet<usize> = HashSet::new();

        // 1. Always keep the most recent snapshot
        retained_indices.insert(0);

        let mut daily_count = 0;
        let mut weekly_count = 0;
        let mut monthly_count = 0;
        let mut yearly_count = 0;

        // Tracks which buckets have already been filled for the specific period to avoid keeping multiple backups for the same period.
        let mut seen_days = HashSet::new();
        let mut seen_weeks = HashSet::new();
        let mut seen_months = HashSet::new();
        let mut seen_years = HashSet::new();

        for (i, (_, dt)) in sorted_snapshots.iter().enumerate() {
            let year = dt.year();
            let month = dt.month();
            let iso_week = dt.iso_week().week();
            let day_of_year = dt.ordinal();

            let day_key = (year, day_of_year);
            let week_key = (dt.iso_week().year(), iso_week);
            let month_key = (year, month);
            let year_key = year;

            let mut kept = false;

            // We want to keep the LATEST backup in a given period.
            // Since we iterate from newest to oldest, the first time we see a period, it is the newest for that period.

            if daily_count < policy.keep_daily && !seen_days.contains(&day_key) {
                seen_days.insert(day_key);
                daily_count += 1;
                kept = true;
            }

            if weekly_count < policy.keep_weekly && !seen_weeks.contains(&week_key) {
                seen_weeks.insert(week_key);
                weekly_count += 1;
                kept = true;
            }

            if monthly_count < policy.keep_monthly && !seen_months.contains(&month_key) {
                seen_months.insert(month_key);
                monthly_count += 1;
                kept = true;
            }

            if yearly_count < policy.keep_yearly && !seen_years.contains(&year_key) {
                seen_years.insert(year_key);
                yearly_count += 1;
                kept = true;
            }

            if kept {
                retained_indices.insert(i);
            }
        }

        let mut retained = Vec::new();
        let mut pruned = Vec::new();

        for (i, (item, _)) in sorted_snapshots.into_iter().enumerate() {
            if retained_indices.contains(&i) {
                retained.push(item);
            } else {
                pruned.push(item);
            }
        }

        GfsPlanResult { retained, pruned }
    }
}
