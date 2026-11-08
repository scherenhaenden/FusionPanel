#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use crate::error::BackupError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InodeKey {
    pub dev: u64,
    pub ino: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotSummary {
    pub path: PathBuf,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub file_count: usize,
    pub apparent_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpaceSavingsSummary {
    pub total_apparent_bytes: u64,
    pub unique_physical_bytes: u64,
    pub saved_bytes: u64,
    pub deduplication_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HardlinkRetentionPolicy {
    pub keep_last: Option<usize>,
    pub keep_daily: Option<u32>,
    pub keep_weekly: Option<u32>,
    pub keep_monthly: Option<u32>,
    pub max_age_days: Option<u32>,
}

impl Default for HardlinkRetentionPolicy {
    fn default() -> Self {
        Self {
            keep_last: Some(7),
            keep_daily: None,
            keep_weekly: None,
            keep_monthly: None,
            max_age_days: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PruningPlan {
    pub retained: Vec<SnapshotSummary>,
    pub to_prune: Vec<SnapshotSummary>,
    pub reclaimable_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PruningResult {
    pub dry_run: bool,
    pub pruned_snapshots: Vec<PathBuf>,
    pub retained_snapshots: Vec<PathBuf>,
    pub reclaimed_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct SnapshotFileRecord {
    pub rel_path: PathBuf,
    pub inode: InodeKey,
    pub size: u64,
    pub nlink: u64,
}

#[derive(Debug, Clone)]
pub struct HardlinkSnapshot {
    pub summary: SnapshotSummary,
    pub files: Vec<SnapshotFileRecord>,
}

impl HardlinkSnapshot {
    pub fn files(&self) -> &[SnapshotFileRecord] {
        &self.files
    }
}

pub struct HardlinkSnapshotInventory {
    snapshots: Vec<HardlinkSnapshot>,
}

impl HardlinkSnapshotInventory {
    /// Scan a base directory containing hardlinked snapshot folders.
    /// Each subfolder is treated as a snapshot.
    pub fn scan_directory<P: AsRef<Path>>(base_dir: P) -> Result<Self, BackupError> {
        let base_path = base_dir.as_ref();
        if !base_path.exists() {
            return Err(BackupError::SourceNotFound(base_path.display().to_string()));
        }

        let mut snapshots = Vec::new();

        for entry in fs::read_dir(base_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                let metadata = entry.metadata()?;
                let modified = metadata.modified()?;
                let created_at: DateTime<Utc> = modified.into();

                let mut file_records = Vec::new();
                let mut apparent_bytes = 0u64;

                scan_files_recursive(&path, &path, &mut file_records, &mut apparent_bytes)?;

                let summary = SnapshotSummary {
                    path: path.clone(),
                    name,
                    created_at,
                    file_count: file_records.len(),
                    apparent_bytes,
                };

                snapshots.push(HardlinkSnapshot {
                    summary,
                    files: file_records,
                });
            }
        }

        // Sort descending by created_at (newest first)
        snapshots.sort_by(|a, b| b.summary.created_at.cmp(&a.summary.created_at));

        Ok(Self { snapshots })
    }

    pub fn snapshots(&self) -> &[HardlinkSnapshot] {
        &self.snapshots
    }

    /// Calculate total space savings and deduplication achieved via hardlinks.
    pub fn calculate_space_savings(&self) -> SpaceSavingsSummary {
        let mut total_apparent_bytes = 0u64;
        let mut unique_inodes: HashMap<InodeKey, u64> = HashMap::new();

        for snap in &self.snapshots {
            total_apparent_bytes += snap.summary.apparent_bytes;
            for file in &snap.files {
                unique_inodes.entry(file.inode.clone()).or_insert(file.size);
            }
        }

        let unique_physical_bytes: u64 = unique_inodes.values().sum();
        let saved_bytes = total_apparent_bytes.saturating_sub(unique_physical_bytes);
        let deduplication_ratio = if unique_physical_bytes > 0 {
            total_apparent_bytes as f64 / unique_physical_bytes as f64
        } else {
            1.0
        };

        SpaceSavingsSummary {
            total_apparent_bytes,
            unique_physical_bytes,
            saved_bytes,
            deduplication_ratio,
        }
    }

    /// Determine which snapshots to retain and which to prune according to the retention policy,
    /// and accurately calculate physical disk space that will be reclaimed.
    pub fn plan_pruning(
        &self,
        policy: &HardlinkRetentionPolicy,
        reference_time: DateTime<Utc>,
    ) -> PruningPlan {
        let mut retained_indices = HashSet::new();

        // 1. keep_last: Keep most recent N snapshots
        if let Some(keep_last) = policy.keep_last {
            for (idx, _) in self.snapshots.iter().enumerate().take(keep_last) {
                retained_indices.insert(idx);
            }
        }

        // 2. keep_daily: Keep newest snapshot of each day within D days
        if let Some(daily_days) = policy.keep_daily {
            let mut seen_days = HashSet::new();

            for (idx, snap) in self.snapshots.iter().enumerate() {
                let diff_days = (reference_time.date_naive() - snap.summary.created_at.date_naive()).num_days();
                if diff_days >= 0 && diff_days <= daily_days as i64 {
                    let day_key = snap.summary.created_at.date_naive();
                    if seen_days.insert(day_key) {
                        retained_indices.insert(idx);
                    }
                }
            }
        }

        // 3. keep_weekly: Keep newest snapshot of each ISO week within W weeks
        if let Some(weekly_weeks) = policy.keep_weekly {
            let mut seen_weeks = HashSet::new();

            for (idx, snap) in self.snapshots.iter().enumerate() {
                let diff_days = (reference_time.date_naive() - snap.summary.created_at.date_naive()).num_days();
                if diff_days >= 0 && diff_days <= (weekly_weeks as i64 * 7) {
                    let iso_week = snap.summary.created_at.iso_week();
                    let week_key = (iso_week.year(), iso_week.week());
                    if seen_weeks.insert(week_key) {
                        retained_indices.insert(idx);
                    }
                }
            }
        }

        // 4. keep_monthly: Keep newest snapshot of each month within M months
        if let Some(monthly_months) = policy.keep_monthly {
            let mut seen_months = HashSet::new();

            for (idx, snap) in self.snapshots.iter().enumerate() {
                let diff_days = (reference_time.date_naive() - snap.summary.created_at.date_naive()).num_days();
                if diff_days >= 0 && diff_days <= (monthly_months as i64 * 31) {
                    let month_key = (
                        snap.summary.created_at.year(),
                        snap.summary.created_at.month(),
                    );
                    if seen_months.insert(month_key) {
                        retained_indices.insert(idx);
                    }
                }
            }
        }

        // 5. max_age_days: Discard anything older than max_age_days UNLESS protected by keep_last
        if let Some(max_age) = policy.max_age_days {
            let keep_last_limit = policy.keep_last.unwrap_or(0);

            for (idx, snap) in self.snapshots.iter().enumerate() {
                let diff_days = (reference_time.date_naive() - snap.summary.created_at.date_naive()).num_days();
                if diff_days > max_age as i64 && idx >= keep_last_limit {
                    retained_indices.remove(&idx);
                }
            }
        }

        let mut retained = Vec::new();
        let mut to_prune = Vec::new();

        let mut retained_inodes = HashSet::new();

        for (idx, snap) in self.snapshots.iter().enumerate() {
            if retained_indices.contains(&idx) {
                retained.push(snap.summary.clone());
                for file in &snap.files {
                    retained_inodes.insert(file.inode.clone());
                }
            } else {
                to_prune.push(snap.summary.clone());
            }
        }

        // Calculate reclaimable bytes:
        // Unique inodes present in `to_prune` that are NOT present in any `retained` snapshot.
        let mut prune_unique_inodes: HashMap<InodeKey, u64> = HashMap::new();
        for (idx, snap) in self.snapshots.iter().enumerate() {
            if !retained_indices.contains(&idx) {
                for file in &snap.files {
                    if !retained_inodes.contains(&file.inode) {
                        prune_unique_inodes.entry(file.inode.clone()).or_insert(file.size);
                    }
                }
            }
        }

        let reclaimable_bytes: u64 = prune_unique_inodes.values().sum();

        PruningPlan {
            retained,
            to_prune,
            reclaimable_bytes,
        }
    }

    /// Execute the pruning plan. If dry_run is true, directories are not touched.
    pub fn execute_pruning(
        &mut self,
        plan: &PruningPlan,
        dry_run: bool,
    ) -> Result<PruningResult, BackupError> {
        let mut pruned_snapshots = Vec::new();
        let mut retained_snapshots = Vec::new();

        for retained in &plan.retained {
            retained_snapshots.push(retained.path.clone());
        }

        for to_remove in &plan.to_prune {
            if !dry_run {
                if to_remove.path.exists() {
                    fs::remove_dir_all(&to_remove.path)?;
                }
            }
            pruned_snapshots.push(to_remove.path.clone());
        }

        if !dry_run {
            // Remove pruned snapshots from memory
            let pruned_set: HashSet<&PathBuf> = pruned_snapshots.iter().collect();
            self.snapshots.retain(|s| !pruned_set.contains(&s.summary.path));
        }

        Ok(PruningResult {
            dry_run,
            pruned_snapshots,
            retained_snapshots,
            reclaimed_bytes: plan.reclaimable_bytes,
        })
    }
}

fn scan_files_recursive(
    root: &Path,
    current_dir: &Path,
    records: &mut Vec<SnapshotFileRecord>,
    apparent_bytes: &mut u64,
) -> Result<(), BackupError> {
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            scan_files_recursive(root, &path, records, apparent_bytes)?;
        } else if file_type.is_file() {
            let metadata = entry.metadata()?;
            let size = metadata.len();
            let inode = InodeKey {
                dev: metadata.dev(),
                ino: metadata.ino(),
            };
            let nlink = metadata.nlink();

            let rel_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            *apparent_bytes += size;

            records.push(SnapshotFileRecord {
                rel_path,
                inode,
                size,
                nlink,
            });
        }
    }
    Ok(())
}
