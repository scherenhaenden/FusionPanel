#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::time::{Duration, SystemTime};

/// Represents a subnet: /24 for IPv4 and /48 for IPv6.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Subnet {
    V4([u8; 3]),
    V6([u8; 6]),
}

impl Subnet {
    /// Extracts the subnet for a given IP address.
    pub fn from_ip(ip: IpAddr) -> Self {
        match ip {
            IpAddr::V4(addr) => {
                let octets = addr.octets();
                Subnet::V4([octets[0], octets[1], octets[2]])
            }
            IpAddr::V6(addr) => {
                let octets = addr.octets();
                Subnet::V6([
                    octets[0], octets[1], octets[2], octets[3], octets[4], octets[5],
                ])
            }
        }
    }

    /// Converts the subnet to its string CIDR representation.
    pub fn to_cidr_string(&self) -> String {
        match self {
            Subnet::V4(octets) => format!("{}.{}.{}.0/24", octets[0], octets[1], octets[2]),
            Subnet::V6(octets) => format!(
                "{:x}:{:x}:{:x}::/48",
                u16::from_be_bytes([octets[0], octets[1]]),
                u16::from_be_bytes([octets[2], octets[3]]),
                u16::from_be_bytes([octets[4], octets[5]])
            ),
        }
    }
}

/// Monitors fail2ban events and aggregates them by subnet.
pub struct JailAggregator {
    threshold: usize,
    window: Duration,
    events: HashMap<Subnet, Vec<(IpAddr, SystemTime)>>,
    banned_subnets: HashSet<Subnet>,
}

impl JailAggregator {
    /// Creates a new JailAggregator.
    /// `threshold`: Number of distinct IPs in a subnet to trigger a subnet ban.
    /// `window`: The sliding time window for distinct IP bans.
    pub fn new(threshold: usize, window: Duration) -> Self {
        Self {
            threshold,
            window,
            events: HashMap::new(),
            banned_subnets: HashSet::new(),
        }
    }

    /// Records a ban event. If the number of distinct IPs in the subnet within
    /// the sliding window reaches the threshold, returns the subnet CIDR string.
    pub fn record_ban(&mut self, ip: IpAddr, jail: &str, time: SystemTime) -> Option<String> {
        let _ = jail; // Monitors jail but triggers solely by IP threshold.
        let subnet = Subnet::from_ip(ip);

        // If the subnet has already been banned, we do not trigger again.
        if self.banned_subnets.contains(&subnet) {
            return None;
        }

        let entry = self.events.entry(subnet.clone()).or_default();
        entry.push((ip, time));

        // Enforce the sliding time window relative to the new event's time
        entry.retain(|&(_, t)| match time.duration_since(t) {
            Ok(duration) => duration <= self.window,
            Err(_) => true, // `t` is in the future compared to `time`, keep it
        });

        // Count distinct IPs
        let distinct_ips: HashSet<IpAddr> = entry.iter().map(|&(i, _)| i).collect();

        if distinct_ips.len() >= self.threshold {
            self.banned_subnets.insert(subnet.clone());
            self.events.remove(&subnet);
            Some(subnet.to_cidr_string())
        } else {
            None
        }
    }

    /// Purges all events that fall outside the sliding window relative to `now`.
    pub fn purge_expired_events(&mut self, now: SystemTime) {
        self.events.retain(|_, evs| {
            evs.retain(|&(_, t)| match now.duration_since(t) {
                Ok(duration) => duration <= self.window,
                Err(_) => true,
            });
            !evs.is_empty()
        });
    }
}
