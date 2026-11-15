#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum QuotaError {
    #[error("Invalid repquota line format")]
    InvalidFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaData {
    pub user: String,
    pub space_used: u64,
    pub space_soft_limit: u64,
    pub space_hard_limit: u64,
    pub inode_used: u64,
    pub inode_soft_limit: u64,
    pub inode_hard_limit: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Dqblk {
    pub dqb_bhardlimit: u64,
    pub dqb_bsoftlimit: u64,
    pub dqb_curspace: u64,
    pub dqb_ihardlimit: u64,
    pub dqb_isoftlimit: u64,
    pub dqb_curinodes: u64,
    pub dqb_btime: u64,
    pub dqb_itime: u64,
    pub dqb_valid: u32,
}

impl From<&Dqblk> for QuotaData {
    fn from(dq: &Dqblk) -> Self {
        QuotaData {
            user: String::new(),
            space_used: dq.dqb_curspace,
            space_soft_limit: dq.dqb_bsoftlimit,
            space_hard_limit: dq.dqb_bhardlimit,
            inode_used: dq.dqb_curinodes,
            inode_soft_limit: dq.dqb_isoftlimit,
            inode_hard_limit: dq.dqb_ihardlimit,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuotaAlert {
    SpaceHardLimitExceeded(u64, u64),
    SpaceSoftLimitExceeded(u64, u64),
    InodeHardLimitExceeded(u64, u64),
    InodeSoftLimitExceeded(u64, u64),
    InodeThresholdExceeded { used: u64, total: u64, threshold_pct: f64 },
}

pub struct QuotaEnforcer {
    pub inode_soft_threshold_pct: f64,
}

impl QuotaEnforcer {
    pub fn new(inode_soft_threshold_pct: f64) -> Self {
        Self {
            inode_soft_threshold_pct,
        }
    }

    pub fn evaluate(&self, data: &QuotaData) -> Vec<QuotaAlert> {
        let mut alerts = Vec::new();

        if data.space_hard_limit > 0 && data.space_used >= data.space_hard_limit {
            alerts.push(QuotaAlert::SpaceHardLimitExceeded(data.space_used, data.space_hard_limit));
        } else if data.space_soft_limit > 0 && data.space_used >= data.space_soft_limit {
            alerts.push(QuotaAlert::SpaceSoftLimitExceeded(data.space_used, data.space_soft_limit));
        }

        if data.inode_hard_limit > 0 {
            if data.inode_used >= data.inode_hard_limit {
                alerts.push(QuotaAlert::InodeHardLimitExceeded(data.inode_used, data.inode_hard_limit));
            } else if data.inode_soft_limit > 0 && data.inode_used >= data.inode_soft_limit {
                alerts.push(QuotaAlert::InodeSoftLimitExceeded(data.inode_used, data.inode_soft_limit));
            }

            let threshold_limit = (data.inode_hard_limit as f64 * (self.inode_soft_threshold_pct / 100.0)) as u64;
            if data.inode_used >= threshold_limit && data.inode_used < data.inode_hard_limit {
                alerts.push(QuotaAlert::InodeThresholdExceeded {
                    used: data.inode_used,
                    total: data.inode_hard_limit,
                    threshold_pct: self.inode_soft_threshold_pct,
                });
            }
        } else if data.inode_soft_limit > 0 && data.inode_used >= data.inode_soft_limit {
            alerts.push(QuotaAlert::InodeSoftLimitExceeded(data.inode_used, data.inode_soft_limit));
        }

        alerts
    }

    pub fn parse_repquota_line(line: &str) -> Result<QuotaData, QuotaError> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 8 {
            return Err(QuotaError::InvalidFormat);
        }

        let user = parts[0].to_string();
        let status = parts[1];
        if status.len() != 2 {
            return Err(QuotaError::InvalidFormat);
        }
        let mut chars = status.chars();
        let block_status = chars.next().unwrap_or(' ');
        let inode_status = chars.next().unwrap_or(' ');

        if (block_status != '+' && block_status != '-') || (inode_status != '+' && inode_status != '-') {
            return Err(QuotaError::InvalidFormat);
        }

        let mut numbers = Vec::new();
        for part in &parts[2..] {
            if let Ok(num) = part.parse::<u64>() {
                numbers.push(num);
            }
        }

        if numbers.len() < 6 {
            return Err(QuotaError::InvalidFormat);
        }

        Ok(QuotaData {
            user,
            space_used: numbers[0],
            space_soft_limit: numbers[1],
            space_hard_limit: numbers[2],
            inode_used: numbers[3],
            inode_soft_limit: numbers[4],
            inode_hard_limit: numbers[5],
        })
    }
}
