#![forbid(unsafe_code)]

/// Represents a systemd journald configuration and tuning policy
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournaldPolicy {
    pub system_max_use: String,
    pub system_keep_free: String,
    pub max_retention_sec: String,
    pub rate_limit_burst: u32,
}

impl JournaldPolicy {
    /// Create a new JournaldPolicy
    pub fn new(
        system_max_use: impl Into<String>,
        system_keep_free: impl Into<String>,
        max_retention_sec: impl Into<String>,
        rate_limit_burst: u32,
    ) -> Self {
        Self {
            system_max_use: system_max_use.into(),
            system_keep_free: system_keep_free.into(),
            max_retention_sec: max_retention_sec.into(),
            rate_limit_burst,
        }
    }

    /// Generates the /etc/systemd/journald.conf configuration block
    pub fn generate_conf(&self) -> String {
        format!(
            "[Journal]\n\
             SystemMaxUse={}\n\
             SystemKeepFree={}\n\
             MaxRetentionSec={}\n\
             RateLimitBurst={}\n",
            self.system_max_use,
            self.system_keep_free,
            self.max_retention_sec,
            self.rate_limit_burst
        )
    }

    /// Gets the journalctl --vacuum-size parameter string
    pub fn vacuum_size_arg(&self) -> String {
        format!("--vacuum-size={}", self.system_max_use)
    }

    /// Gets the journalctl --vacuum-time parameter string
    pub fn vacuum_time_arg(&self) -> String {
        format!("--vacuum-time={}", self.max_retention_sec)
    }

    /// Calculates disk space reclaimable based on current bytes and target string size
    pub fn calculate_reclaimable_space(&self, current_size_bytes: u64) -> u64 {
        let max_size = parse_size_to_bytes(&self.system_max_use).unwrap_or(u64::MAX);
        current_size_bytes.saturating_sub(max_size)
    }
}

/// Parses a human-readable size (e.g., "1G", "500M") into bytes.
pub fn parse_size_to_bytes(size: &str) -> Option<u64> {
    let size = size.trim().to_uppercase();
    if size.is_empty() {
        return None;
    }

    let mut number_part = String::new();
    let mut unit_part = String::new();

    for c in size.chars() {
        if c.is_ascii_digit() || c == '.' {
            number_part.push(c);
        } else {
            unit_part.push(c);
        }
    }

    let value: f64 = number_part.parse().ok()?;

    let multiplier: f64 = match unit_part.as_str() {
        "" | "B" => 1.0,
        "K" | "KB" => 1024.0,
        "M" | "MB" => 1024.0 * 1024.0,
        "G" | "GB" => 1024.0 * 1024.0 * 1024.0,
        "T" | "TB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        "P" | "PB" => 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };

    Some((value * multiplier) as u64)
}
