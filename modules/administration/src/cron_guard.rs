#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use chrono::{DateTime, Utc, TimeZone, Datelike, Timelike};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq)]
pub struct CronSchedule {
    pub minutes: BTreeSet<u32>,
    pub hours: BTreeSet<u32>,
    pub days_of_month: BTreeSet<u32>,
    pub months: BTreeSet<u32>,
    pub days_of_week: BTreeSet<u32>,
    pub dom_restricted: bool,
    pub dow_restricted: bool,
}

pub fn calculate_jitter(seed: &str) -> u32 {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let result = hasher.finalize();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&result[0..8]);
    let hash_val = u64::from_be_bytes(bytes);
    (hash_val % 60) as u32
}

impl CronSchedule {
    pub fn parse(expr: &str) -> Result<Self, String> {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() != 5 {
            return Err("Invalid cron expression: must have exactly 5 fields".to_string());
        }

        let minutes = parse_field(parts[0], 0, 59)?;
        let hours = parse_field(parts[1], 0, 23)?;
        let days_of_month = parse_field(parts[2], 1, 31)?;
        let months = parse_field(parts[3], 1, 12)?;
        let days_of_week = parse_field(parts[4], 0, 7)?;

        let dom_restricted = parts[2] != "*";
        let dow_restricted = parts[4] != "*";

        Ok(CronSchedule {
            minutes,
            hours,
            days_of_month,
            months,
            days_of_week,
            dom_restricted,
            dow_restricted,
        })
    }

    pub fn parse_with_jitter(expr: &str, jitter_seed: &str) -> Result<Self, String> {
        let mut schedule = Self::parse(expr)?;

        let jitter = calculate_jitter(jitter_seed);
        let mut new_minutes = BTreeSet::new();
        for &m in &schedule.minutes {
            new_minutes.insert((m + jitter) % 60);
        }
        schedule.minutes = new_minutes;

        Ok(schedule)
    }

    pub fn next_execution(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        // Start from next minute
        let mut current = after.with_second(0).unwrap().with_nanosecond(0).unwrap() + chrono::Duration::minutes(1);

        for _ in 0..10_000 {
            if !self.months.contains(&(current.month() as u32)) {
                current = advance_month(current);
                continue;
            }

            let dom_match = self.days_of_month.contains(&(current.day() as u32));
            let current_dow = current.weekday().num_days_from_sunday() as u32;
            let dow_match = self.days_of_week.contains(&current_dow) || (current_dow == 0 && self.days_of_week.contains(&7));

            let day_match = if self.dom_restricted && self.dow_restricted {
                dom_match || dow_match
            } else {
                dom_match && dow_match
            };

            if !day_match {
                current = advance_day(current);
                continue;
            }

            if !self.hours.contains(&(current.hour() as u32)) {
                current = advance_hour(current);
                continue;
            }

            if !self.minutes.contains(&(current.minute() as u32)) {
                current = current + chrono::Duration::minutes(1);
                continue;
            }

            return Some(current);
        }

        None
    }
}

fn parse_field(field: &str, min: u32, max: u32) -> Result<BTreeSet<u32>, String> {
    let mut values = BTreeSet::new();

    for part in field.split(',') {
        if part == "*" {
            for i in min..=max {
                values.insert(i);
            }
            continue;
        }

        if let Some(slash_idx) = part.find('/') {
            let range_part = &part[..slash_idx];
            let step_part = &part[slash_idx + 1..];

            let step: u32 = step_part.parse().map_err(|_| "Invalid step".to_string())?;
            if step == 0 {
                return Err("Step cannot be 0".to_string());
            }

            let (start, end) = if range_part == "*" {
                (min, max)
            } else if let Some(dash_idx) = range_part.find('-') {
                let start: u32 = range_part[..dash_idx].parse().map_err(|_| "Invalid range start".to_string())?;
                let end: u32 = range_part[dash_idx + 1..].parse().map_err(|_| "Invalid range end".to_string())?;
                (start, end)
            } else {
                let start: u32 = range_part.parse().map_err(|_| "Invalid range start".to_string())?;
                (start, max)
            };

            if start < min || end > max || start > end {
                return Err("Invalid range bounds".to_string());
            }

            let mut current = start;
            while current <= end {
                values.insert(current);
                current += step;
            }
        } else if let Some(dash_idx) = part.find('-') {
            let start: u32 = part[..dash_idx].parse().map_err(|_| "Invalid range start".to_string())?;
            let end: u32 = part[dash_idx + 1..].parse().map_err(|_| "Invalid range end".to_string())?;

            if start < min || end > max || start > end {
                return Err("Invalid range bounds".to_string());
            }

            for i in start..=end {
                values.insert(i);
            }
        } else {
            let val: u32 = part.parse().map_err(|_| "Invalid value".to_string())?;
            if val < min || val > max {
                return Err(format!("Value {} out of bounds {}-{}", val, min, max));
            }
            values.insert(val);
        }
    }

    Ok(values)
}

fn advance_month(dt: DateTime<Utc>) -> DateTime<Utc> {
    let mut year = dt.year();
    let mut month = dt.month() + 1;
    if month > 12 {
        month = 1;
        year += 1;
    }
    Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0).unwrap()
}

fn advance_day(dt: DateTime<Utc>) -> DateTime<Utc> {
    (dt + chrono::Duration::days(1)).with_hour(0).unwrap().with_minute(0).unwrap()
}

fn advance_hour(dt: DateTime<Utc>) -> DateTime<Utc> {
    (dt + chrono::Duration::hours(1)).with_minute(0).unwrap()
}
