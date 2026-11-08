#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinlogFile {
    pub filename: String,
    pub size_bytes: u64,
    pub timestamp_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionRules {
    pub max_total_size_bytes: Option<u64>,
    pub max_age_seconds: Option<u64>,
    pub minimum_retained_logs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurgePlan {
    pub cutoff_filename: String,
    pub sql_command: String,
}

pub struct BinlogRetentionCalculator;

impl BinlogRetentionCalculator {
    pub fn calculate(
        logs: Vec<BinlogFile>,
        rules: &RetentionRules,
        current_time_seconds: u64,
    ) -> Option<PurgePlan> {
        if logs.is_empty() {
            return None;
        }

        let mut sorted_logs = logs;
        sorted_logs.sort_by(|a, b| {
            let seq_a = Self::extract_sequence(&a.filename);
            let seq_b = Self::extract_sequence(&b.filename);
            seq_a.cmp(&seq_b).then(a.filename.cmp(&b.filename))
        });

        let total_logs = sorted_logs.len();
        let min_kept = std::cmp::max(1, rules.minimum_retained_logs);

        let mut keep_count = 0;
        let mut accumulated_size = 0;

        for log in sorted_logs.iter().rev() {
            let is_min_kept = keep_count < min_kept;

            if !is_min_kept {
                if let Some(max_size) = rules.max_total_size_bytes {
                    if accumulated_size + log.size_bytes > max_size {
                        break;
                    }
                }

                if let Some(max_age) = rules.max_age_seconds {
                    let age = current_time_seconds.saturating_sub(log.timestamp_seconds);
                    if age > max_age {
                        break;
                    }
                }
            }

            keep_count += 1;
            accumulated_size += log.size_bytes;
        }

        let purge_count = total_logs.saturating_sub(keep_count);

        if purge_count == 0 {
            return None;
        }

        let cutoff_file = &sorted_logs[purge_count];
        Some(PurgePlan {
            cutoff_filename: cutoff_file.filename.clone(),
            sql_command: format!("PURGE BINARY LOGS TO '{}';", cutoff_file.filename),
        })
    }

    fn extract_sequence(filename: &str) -> Option<u64> {
        let parts: Vec<&str> = filename.split('.').collect();
        if let Some(ext) = parts.last() {
            ext.parse::<u64>().ok()
        } else {
            None
        }
    }
}
