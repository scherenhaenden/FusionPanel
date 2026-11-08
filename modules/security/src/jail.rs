#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use thiserror::Error;

use std::net::IpAddr;
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq)]
pub enum JailAction {
    Found,
    Ban,
    Unban,
    Restore,
}

impl FromStr for JailAction {
    type Err = JailError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Found" => Ok(JailAction::Found),
            "Ban" => Ok(JailAction::Ban),
            "Unban" => Ok(JailAction::Unban),
            "Restore" => Ok(JailAction::Restore),
            _ => Err(JailError::MalformedLog),
        }
    }
}

#[derive(Debug)]
pub struct JailLogEvent {
    pub jail: String,
    pub action: JailAction,
    pub ip: IpAddr,
}

impl JailLogEvent {
    /// Parse a single fail2ban log line
    pub fn parse(line: &str) -> Result<Self, JailError> {
        if line.len() > 1024 {
            return Err(JailError::LineTooLong);
        }

        let parts: Vec<&str> = line.split_whitespace().collect();

        let mut jail_name = None;
        let mut action_name = None;
        let mut ip_str = None;

        for (i, part) in parts.iter().enumerate() {
            if part.starts_with('[') && part.ends_with(']') {
                let possible_jail = &part[1..part.len() - 1];
                if i + 1 < parts.len() {
                    let next_word = parts[i + 1];
                    if next_word == "Ban"
                        || next_word == "Unban"
                        || next_word == "Found"
                        || next_word == "Restore"
                        || next_word == "RestoreBan"
                    {
                        jail_name = Some(possible_jail.to_string());

                        // Treat RestoreBan same as Restore (or Ban if preferred? let's stick to simple ones first)
                        let action_val = if next_word == "RestoreBan" {
                            "Restore"
                        } else {
                            next_word
                        };
                        action_name = Some(action_val.to_string());

                        if i + 2 < parts.len() {
                            ip_str = Some(parts[i + 2].to_string());
                        }
                        break;
                    }
                }
            }
        }

        let jail = jail_name.ok_or(JailError::MalformedLog)?;
        let action_str = action_name.ok_or(JailError::MalformedLog)?;
        let ip_s = ip_str.ok_or(JailError::MalformedLog)?;

        let action = JailAction::from_str(&action_str)?;
        let ip = IpAddr::from_str(&ip_s).map_err(|_| JailError::InvalidIp)?;

        Ok(Self { jail, action, ip })
    }
}

#[derive(Debug, Error)]
pub enum JailError {
    #[error("IP is currently banned")]
    IpBanned,
    #[error("IP is not currently banned")]
    IpNotBanned,
    #[error("Log line exceeds maximum allowed length")]
    LineTooLong,
    #[error("Malformed fail2ban log line")]
    MalformedLog,
    #[error("Invalid IP address")]
    InvalidIp,
}

#[derive(Debug, Clone)]
pub struct JailConfig {
    pub maxretry: u32,
    pub findtime: Duration,
    pub bantime: Duration,
    pub subnet_ban_threshold: u32,
}

impl Default for JailConfig {
    fn default() -> Self {
        Self {
            maxretry: 3,
            findtime: Duration::from_secs(600), // 10 minutes
            bantime: Duration::from_secs(3600), // 1 hour
            subnet_ban_threshold: 5,
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Subnet {
    V4([u8; 3]), // /24
    V6([u8; 6]), // /48
}

impl Subnet {
    pub fn from_ip(ip: IpAddr) -> Self {
        match ip {
            IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();
                Subnet::V4([octets[0], octets[1], octets[2]])
            }
            IpAddr::V6(ipv6) => {
                let octets = ipv6.octets();
                Subnet::V6([
                    octets[0], octets[1], octets[2], octets[3], octets[4], octets[5],
                ])
            }
        }
    }
}

pub struct JailManager {
    config: JailConfig,
    // jail -> ip -> failed attempts
    failed_attempts: HashMap<String, HashMap<IpAddr, Vec<SystemTime>>>,
    // jail -> ip -> expire time
    banned_ips: HashMap<String, HashMap<IpAddr, SystemTime>>,
    // jail -> subnet -> expire time
    banned_subnets: HashMap<String, HashMap<Subnet, SystemTime>>,
}

impl JailManager {
    pub fn new(config: JailConfig) -> Self {
        Self {
            config,
            failed_attempts: HashMap::new(),
            banned_ips: HashMap::new(),
            banned_subnets: HashMap::new(),
        }
    }

    /// Clean up expired bans and old failed attempts
    pub fn cleanup(&mut self, now: SystemTime) {
        // Remove expired bans
        for (_, bans) in self.banned_ips.iter_mut() {
            bans.retain(|_, &mut expire_time| expire_time > now);
        }
        self.banned_ips.retain(|_, bans| !bans.is_empty());

        for (_, bans) in self.banned_subnets.iter_mut() {
            bans.retain(|_, &mut expire_time| expire_time > now);
        }
        self.banned_subnets.retain(|_, bans| !bans.is_empty());

        // Remove old failed attempts outside of findtime
        let findtime = self.config.findtime;
        for (_, jail_attempts) in self.failed_attempts.iter_mut() {
            jail_attempts.retain(|_, attempts| {
                attempts.retain(|&attempt_time| {
                    if let Ok(elapsed) = now.duration_since(attempt_time) {
                        elapsed <= findtime
                    } else {
                        false // if attempt time is in the future, remove it just in case
                    }
                });
                !attempts.is_empty()
            });
        }
        self.failed_attempts
            .retain(|_, jail_attempts| !jail_attempts.is_empty());
    }

    /// Process a parsed jail log event
    pub fn process_event(&mut self, event: JailLogEvent, now: SystemTime) -> Result<(), JailError> {
        self.cleanup(now);
        match event.action {
            JailAction::Found => {
                let _ = self.record_failure(&event.jail, event.ip, now);
            }
            JailAction::Ban | JailAction::Restore => {
                self.ban(&event.jail, event.ip, now);
            }
            JailAction::Unban => {
                let _ = self.unban(&event.jail, event.ip);
            }
        }
        Ok(())
    }

    /// Record a failed authentication attempt
    pub fn record_failure(
        &mut self,
        jail: &str,
        ip: IpAddr,
        now: SystemTime,
    ) -> Result<bool, JailError> {
        self.cleanup(now);

        if self.is_banned(jail, ip, now) {
            return Ok(true); // Already banned
        }

        let jail_attempts = self
            .failed_attempts
            .entry(jail.to_string())
            .or_insert_with(HashMap::new);
        let attempts = jail_attempts.entry(ip).or_insert_with(Vec::new);
        attempts.push(now);

        if attempts.len() as u32 >= self.config.maxretry {
            // Trigger ban
            self.ban(jail, ip, now);
            return Ok(true); // Was banned
        }

        Ok(false) // Not banned
    }

    fn check_subnet_ban(&mut self, jail: &str, subnet: Subnet, now: SystemTime) {
        if let Some(bans) = self.banned_ips.get(jail) {
            let count = bans
                .keys()
                .filter(|&&ip| Subnet::from_ip(ip) == subnet)
                .count() as u32;
            if count >= self.config.subnet_ban_threshold {
                let expire_time = now + self.config.bantime;
                let jail_subnets = self
                    .banned_subnets
                    .entry(jail.to_string())
                    .or_insert_with(HashMap::new);
                jail_subnets.insert(subnet.clone(), expire_time);
            }
        }
    }

    /// Check if an IP is currently banned in a specific jail
    pub fn is_banned(&mut self, jail: &str, ip: IpAddr, now: SystemTime) -> bool {
        self.cleanup(now);

        if let Some(subnets) = self.banned_subnets.get(jail) {
            let subnet = Subnet::from_ip(ip);
            if subnets.contains_key(&subnet) {
                return true;
            }
        }

        if let Some(bans) = self.banned_ips.get(jail) {
            if bans.contains_key(&ip) {
                return true;
            }
        }

        false
    }

    /// Manually ban an IP address in a jail
    pub fn ban(&mut self, jail: &str, ip: IpAddr, now: SystemTime) {
        let expire_time = now + self.config.bantime;
        let jail_bans = self
            .banned_ips
            .entry(jail.to_string())
            .or_insert_with(HashMap::new);
        jail_bans.insert(ip, expire_time);

        if let Some(jail_attempts) = self.failed_attempts.get_mut(jail) {
            jail_attempts.remove(&ip);
        }

        let subnet = Subnet::from_ip(ip);
        self.check_subnet_ban(jail, subnet, now);
    }

    /// Manually unban an IP address in a jail
    pub fn unban(&mut self, jail: &str, ip: IpAddr) -> Result<(), JailError> {
        let mut unbanned = false;
        if let Some(jail_bans) = self.banned_ips.get_mut(jail) {
            if jail_bans.remove(&ip).is_some() {
                unbanned = true;
            }
        }

        let subnet = Subnet::from_ip(ip);
        if let Some(jail_subnets) = self.banned_subnets.get_mut(jail) {
            if jail_subnets.remove(&subnet).is_some() {
                unbanned = true;
            }
        }

        if unbanned {
            Ok(())
        } else {
            Err(JailError::IpNotBanned)
        }
    }

    /// Get active ban list for all jails. Returns a list of strings "jail:ip" or "jail:subnet" for testing purposes
    pub fn active_bans(&mut self, now: SystemTime) -> Vec<String> {
        self.cleanup(now);
        let mut result = Vec::new();

        for (jail, bans) in &self.banned_ips {
            for ip in bans.keys() {
                result.push(format!("{}:{}", jail, ip));
            }
        }

        for (jail, subnets) in &self.banned_subnets {
            for subnet in subnets.keys() {
                let subnet_str = match subnet {
                    Subnet::V4(octets) => format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2]),
                    Subnet::V6(octets) => format!(
                        "{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}::/48",
                        octets[0], octets[1], octets[2], octets[3], octets[4], octets[5]
                    ),
                };
                result.push(format!("{}:subnet:{}", jail, subnet_str));
            }
        }

        result
    }
}
