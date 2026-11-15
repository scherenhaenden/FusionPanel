#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum SentinelError {
    #[error("Cluster is at risk of split-brain: {0}")]
    SplitBrainRisk(String),
    #[error("Missing authentication: {0}")]
    MissingAuth(String),
    #[error("Invalid quorum setting: {0}")]
    InvalidQuorum(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SentinelInstance {
    pub id: String,
    pub ip: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplicaConfig {
    pub id: String,
    pub ip: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MasterConfig {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub auth_pass: Option<String>,
    pub down_after_milliseconds: u64,
    pub parallel_syncs: u32,
    pub failover_timeout: u64,
    pub configured_quorum: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct SentinelClusterManager {
    pub master: MasterConfig,
    pub replicas: Vec<ReplicaConfig>,
    pub sentinels: Vec<SentinelInstance>,
}

impl SentinelClusterManager {
    pub fn new(master: MasterConfig) -> Self {
        Self {
            master,
            replicas: Vec::new(),
            sentinels: Vec::new(),
        }
    }

    pub fn add_sentinel(&mut self, sentinel: SentinelInstance) {
        self.sentinels.push(sentinel);
    }

    pub fn add_replica(&mut self, replica: ReplicaConfig) {
        self.replicas.push(replica);
    }

    /// Calculates the optimal quorum based on the number of sentinels (N/2 + 1).
    pub fn calculate_optimal_quorum(&self) -> u32 {
        (self.sentinels.len() as u32) / 2 + 1
    }

    /// Validates the topology for split-brain risks, missing auth, and invalid quorum.
    pub fn validate_topology(&self) -> Result<(), SentinelError> {
        let n = self.sentinels.len();

        if n < 3 {
            return Err(SentinelError::SplitBrainRisk("At least 3 sentinels are required for a robust cluster".to_string()));
        }

        if n % 2 == 0 {
            return Err(SentinelError::SplitBrainRisk("An odd number of sentinels is recommended to avoid split-brain".to_string()));
        }

        let optimal_quorum = self.calculate_optimal_quorum();
        if let Some(q) = self.master.configured_quorum {
            if q != optimal_quorum {
                return Err(SentinelError::InvalidQuorum(format!("Configured quorum {} does not match optimal quorum {}", q, optimal_quorum)));
            }
        }

        if self.master.auth_pass.is_none() {
            return Err(SentinelError::MissingAuth("Master requires an auth-pass for secure sentinel communication".to_string()));
        }

        Ok(())
    }

    /// Generates the `sentinel.conf` content for a sentinel instance monitoring this master.
    pub fn generate_sentinel_conf(&self) -> String {
        let quorum = self.master.configured_quorum.unwrap_or_else(|| self.calculate_optimal_quorum());
        let master_name = &self.master.name;

        let mut lines = Vec::new();

        lines.push(format!("sentinel monitor {} {} {} {}", master_name, self.master.ip, self.master.port, quorum));

        if let Some(auth_pass) = &self.master.auth_pass {
            lines.push(format!("sentinel auth-pass {} {}", master_name, auth_pass));
        }

        lines.push(format!("sentinel down-after-milliseconds {} {}", master_name, self.master.down_after_milliseconds));
        lines.push(format!("sentinel parallel-syncs {} {}", master_name, self.master.parallel_syncs));
        lines.push(format!("sentinel failover-timeout {} {}", master_name, self.master.failover_timeout));

        lines.join("\n")
    }

    /// Parses a `sentinel.conf` configuration into a `MasterConfig`.
    pub fn parse_sentinel_conf(conf: &str) -> Result<MasterConfig, SentinelError> {
        let mut name = None;
        let mut ip = None;
        let mut port = None;
        let mut configured_quorum = None;
        let mut auth_pass = None;
        let mut down_after_milliseconds = None;
        let mut parallel_syncs = None;
        let mut failover_timeout = None;

        for line in conf.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            if parts[0] == "sentinel" {
                if parts.len() < 4 {
                    return Err(SentinelError::InvalidConfig(format!("Invalid sentinel config line: {}", line)));
                }

                match parts[1] {
                    "monitor" => {
                        if parts.len() != 6 {
                            return Err(SentinelError::InvalidConfig(format!("Invalid monitor line: {}", line)));
                        }
                        name = Some(parts[2].to_string());
                        ip = Some(parts[3].to_string());
                        port = Some(parts[4].parse().map_err(|_| SentinelError::InvalidConfig(format!("Invalid port: {}", parts[4])))?);
                        configured_quorum = Some(parts[5].parse().map_err(|_| SentinelError::InvalidConfig(format!("Invalid quorum: {}", parts[5])))?);
                    }
                    "auth-pass" => {
                        if parts.len() != 4 {
                            return Err(SentinelError::InvalidConfig(format!("Invalid auth-pass line: {}", line)));
                        }
                        auth_pass = Some(parts[3].to_string());
                    }
                    "down-after-milliseconds" => {
                        if parts.len() != 4 {
                            return Err(SentinelError::InvalidConfig(format!("Invalid down-after-milliseconds line: {}", line)));
                        }
                        down_after_milliseconds = Some(parts[3].parse().map_err(|_| SentinelError::InvalidConfig(format!("Invalid ms: {}", parts[3])))?);
                    }
                    "parallel-syncs" => {
                        if parts.len() != 4 {
                            return Err(SentinelError::InvalidConfig(format!("Invalid parallel-syncs line: {}", line)));
                        }
                        parallel_syncs = Some(parts[3].parse().map_err(|_| SentinelError::InvalidConfig(format!("Invalid syncs: {}", parts[3])))?);
                    }
                    "failover-timeout" => {
                        if parts.len() != 4 {
                            return Err(SentinelError::InvalidConfig(format!("Invalid failover-timeout line: {}", line)));
                        }
                        failover_timeout = Some(parts[3].parse().map_err(|_| SentinelError::InvalidConfig(format!("Invalid failover-timeout: {}", parts[3])))?);
                    }
                    _ => {}
                }
            }
        }

        let name = name.ok_or_else(|| SentinelError::InvalidConfig("Missing monitor directive".to_string()))?;
        let ip = ip.ok_or_else(|| SentinelError::InvalidConfig("Missing master ip".to_string()))?;
        let port = port.ok_or_else(|| SentinelError::InvalidConfig("Missing master port".to_string()))?;

        Ok(MasterConfig {
            name,
            ip,
            port,
            auth_pass,
            down_after_milliseconds: down_after_milliseconds.unwrap_or(30000),
            parallel_syncs: parallel_syncs.unwrap_or(1),
            failover_timeout: failover_timeout.unwrap_or(180000),
            configured_quorum,
        })
    }
}
