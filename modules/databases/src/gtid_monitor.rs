#![forbid(unsafe_code)]

use std::collections::{BTreeMap,  };
use std::fmt;
use std::str::FromStr;
use std::time::{Duration, Instant};

/// Represents a MariaDB GTID (domain_id, server_id, sequence_no)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Gtid {
    pub domain_id: u32,
    pub server_id: u32,
    pub sequence_no: u64,
}

impl fmt::Display for Gtid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}-{}", self.domain_id, self.server_id, self.sequence_no)
    }
}

impl FromStr for Gtid {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid GTID format: {}", s));
        }

        let domain_id = u32::from_str(parts[0]).map_err(|_| format!("Invalid domain_id: {}", parts[0]))?;
        let server_id = u32::from_str(parts[1]).map_err(|_| format!("Invalid server_id: {}", parts[1]))?;
        let sequence_no = u64::from_str(parts[2]).map_err(|_| format!("Invalid sequence_no: {}", parts[2]))?;

        Ok(Gtid {
            domain_id,
            server_id,
            sequence_no,
        })
    }
}

/// Represents a set of GTIDs from different domains/servers
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GtidSet {
    /// Maps (domain_id, server_id) -> max sequence_no
    pub gtids: BTreeMap<(u32, u32), u64>,
}

impl fmt::Display for GtidSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut gtids: Vec<String> = self
            .gtids
            .iter()
            .map(|(&(d, s), &seq)| format!("{}-{}-{}", d, s, seq))
            .collect();
        gtids.sort();
        write!(f, "{}", gtids.join(","))
    }
}

impl FromStr for GtidSet {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut gtids = BTreeMap::new();
        if s.trim().is_empty() {
            return Ok(GtidSet { gtids });
        }

        for part in s.split(',') {
            let gtid = Gtid::from_str(part.trim())?;
            let key = (gtid.domain_id, gtid.server_id);
            // MariaDB GTID sets might have multiple entries for the same domain-server,
            // we keep the maximum sequence number.
            let current = gtids.entry(key).or_insert(0);
            if gtid.sequence_no > *current {
                *current = gtid.sequence_no;
            }
        }

        Ok(GtidSet { gtids })
    }
}

impl GtidSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a GTID to the set, updating the sequence number if it's higher.
    pub fn add(&mut self, gtid: Gtid) {
        let key = (gtid.domain_id, gtid.server_id);
        let current = self.gtids.entry(key).or_insert(0);
        if gtid.sequence_no > *current {
            *current = gtid.sequence_no;
        }
    }

    /// Checks if this set is a subset of `other`.
    /// This means for every (domain, server) in `self`, it exists in `other`
    /// and `self.sequence_no <= other.sequence_no`.
    pub fn is_subset_of(&self, other: &Self) -> bool {
        for (key, &seq) in &self.gtids {
            if let Some(&other_seq) = other.gtids.get(key) {
                if seq > other_seq {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    /// Calculates the sequence gap between this set (acting as master) and `replica`.
    /// Returns a map of (domain, server) -> missing sequence counts.
    /// If replica is ahead for some domain/server, gap is 0.
    pub fn calculate_gap(&self, replica: &Self) -> BTreeMap<(u32, u32), u64> {
        let mut gaps = BTreeMap::new();
        for (key, &master_seq) in &self.gtids {
            let replica_seq = replica.gtids.get(key).copied().unwrap_or(0);
            if master_seq > replica_seq {
                gaps.insert(*key, master_seq - replica_seq);
            }
        }
        gaps
    }

    /// Calculates total replica lag (sum of all sequence gaps).
    pub fn calculate_total_lag(&self, replica: &Self) -> u64 {
        self.calculate_gap(replica).values().sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectorState {
    Healthy,
    Degraded,
    Unreachable,
    Quarantined,
}

#[derive(Debug, Clone)]
pub struct MasterFailureDetector {
    state: DetectorState,
    last_heartbeat: Option<Instant>,
    degraded_timeout: Duration,
    unreachable_timeout: Duration,
    quarantine_timeout: Duration,
}

impl MasterFailureDetector {
    pub fn new(
        degraded_timeout: Duration,
        unreachable_timeout: Duration,
        quarantine_timeout: Duration,
    ) -> Self {
        Self {
            state: DetectorState::Healthy,
            last_heartbeat: Some(Instant::now()),
            degraded_timeout,
            unreachable_timeout,
            quarantine_timeout,
        }
    }

    pub fn record_heartbeat(&mut self) {
        self.last_heartbeat = Some(Instant::now());
        self.state = DetectorState::Healthy;
    }

    pub fn state(&self) -> DetectorState {
        self.state
    }

    pub fn update_state(&mut self) -> DetectorState {
        let now = Instant::now();
        if let Some(last) = self.last_heartbeat {
            let elapsed = now.duration_since(last);
            if elapsed >= self.quarantine_timeout {
                self.state = DetectorState::Quarantined;
            } else if elapsed >= self.unreachable_timeout {
                if self.state != DetectorState::Quarantined {
                    self.state = DetectorState::Unreachable;
                }
            } else if elapsed >= self.degraded_timeout {
                if self.state == DetectorState::Healthy {
                    self.state = DetectorState::Degraded;
                }
            }
        }
        self.state
    }

    // Evaluate if a replica is desynchronized based on lag thresholds
    pub fn check_desynchronization(master_set: &GtidSet, replica_set: &GtidSet, max_lag: u64) -> bool {
        let lag = master_set.calculate_total_lag(replica_set);
        lag > max_lag
    }
}
