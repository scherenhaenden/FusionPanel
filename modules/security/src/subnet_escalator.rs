#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cidr {
    pub ip: IpAddr,
    pub prefix: u8,
}

impl Cidr {
    pub fn new(ip: IpAddr, prefix: u8) -> Self {
        Self { ip, prefix }
    }

    pub fn contains(&self, other_ip: IpAddr) -> bool {
        match (self.ip, other_ip) {
            (IpAddr::V4(a), IpAddr::V4(b)) => {
                if self.prefix == 0 { return true; }
                if self.prefix >= 32 {
                    return a == b;
                }
                let mask = u32::MAX << (32 - self.prefix);
                let a_u32 = u32::from_be_bytes(a.octets());
                let b_u32 = u32::from_be_bytes(b.octets());
                (a_u32 & mask) == (b_u32 & mask)
            }
            (IpAddr::V6(a), IpAddr::V6(b)) => {
                if self.prefix == 0 { return true; }
                if self.prefix >= 128 {
                    return a == b;
                }
                let mask = u128::MAX << (128 - self.prefix);
                let a_u128 = u128::from_be_bytes(a.octets());
                let b_u128 = u128::from_be_bytes(b.octets());
                (a_u128 & mask) == (b_u128 & mask)
            }
            _ => false,
        }
    }

    pub fn intersects(&self, other: &Cidr) -> bool {
        match (self.ip, other.ip) {
            (IpAddr::V4(a), IpAddr::V4(b)) => {
                let min_prefix = std::cmp::min(self.prefix, other.prefix);
                if min_prefix == 0 { return true; }
                if min_prefix >= 32 {
                    return a == b;
                }
                let mask = u32::MAX << (32 - min_prefix);
                let a_u32 = u32::from_be_bytes(a.octets());
                let b_u32 = u32::from_be_bytes(b.octets());
                (a_u32 & mask) == (b_u32 & mask)
            }
            (IpAddr::V6(a), IpAddr::V6(b)) => {
                let min_prefix = std::cmp::min(self.prefix, other.prefix);
                if min_prefix == 0 { return true; }
                if min_prefix >= 128 {
                    return a == b;
                }
                let mask = u128::MAX << (128 - min_prefix);
                let a_u128 = u128::from_be_bytes(a.octets());
                let b_u128 = u128::from_be_bytes(b.octets());
                (a_u128 & mask) == (b_u128 & mask)
            }
            _ => false,
        }
    }
}

impl FromStr for Cidr {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() == 1 {
            let ip = IpAddr::from_str(parts[0]).map_err(|e| e.to_string())?;
            let prefix = match ip {
                IpAddr::V4(_) => 32,
                IpAddr::V6(_) => 128,
            };
            Ok(Self::new(ip, prefix))
        } else if parts.len() == 2 {
            let ip = IpAddr::from_str(parts[0]).map_err(|e| e.to_string())?;
            let prefix = parts[1].parse::<u8>().map_err(|e| e.to_string())?;
            match ip {
                IpAddr::V4(_) => {
                    if prefix > 32 {
                        return Err("Invalid IPv4 prefix".to_string());
                    }
                }
                IpAddr::V6(_) => {
                    if prefix > 128 {
                        return Err("Invalid IPv6 prefix".to_string());
                    }
                }
            }
            Ok(Self::new(ip, prefix))
        } else {
            Err("Invalid CIDR format".to_string())
        }
    }
}

pub fn get_subnet(ip: IpAddr) -> Cidr {
    match ip {
        IpAddr::V4(a) => {
            let mask = u32::MAX << (32 - 24);
            let a_u32 = u32::from_be_bytes(a.octets());
            let subnet_ip = Ipv4Addr::from((a_u32 & mask).to_be_bytes());
            Cidr::new(IpAddr::V4(subnet_ip), 24)
        }
        IpAddr::V6(a) => {
            let mask = u128::MAX << (128 - 64);
            let a_u128 = u128::from_be_bytes(a.octets());
            let subnet_ip = Ipv6Addr::from((a_u128 & mask).to_be_bytes());
            Cidr::new(IpAddr::V6(subnet_ip), 64)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscalationResult {
    Escalated {
        subnet: Cidr,
        duration: Duration,
    },
    NotEscalated,
}

#[derive(Debug, Clone)]
pub struct BanEvent {
    pub ip: IpAddr,
    pub timestamp: u64,
}

pub struct SubnetEscalator {
    threshold: usize,
    time_window_sec: u64,
    escalation_multiplier: u32,
    whitelist: Vec<Cidr>,
    history: HashMap<Cidr, VecDeque<BanEvent>>,
}

impl SubnetEscalator {
    pub fn new(
        threshold: usize,
        time_window_sec: u64,
        escalation_multiplier: u32,
        whitelist: Vec<Cidr>,
    ) -> Self {
        Self {
            threshold,
            time_window_sec,
            escalation_multiplier,
            whitelist,
            history: HashMap::new(),
        }
    }

    pub fn register_ban(
        &mut self,
        ip: IpAddr,
        timestamp: u64,
        original_duration: Duration,
    ) -> EscalationResult {
        let subnet = get_subnet(ip);

        // Does subnet intersect whitelist?
        for whitelisted in &self.whitelist {
            if subnet.intersects(whitelisted) {
                return EscalationResult::NotEscalated;
            }
        }

        // Add to history and prune
        let events = self.history.entry(subnet).or_insert_with(VecDeque::new);
        events.push_back(BanEvent { ip, timestamp });

        let window_start = timestamp.saturating_sub(self.time_window_sec);

        while let Some(event) = events.front() {
            if event.timestamp < window_start {
                events.pop_front();
            } else {
                break;
            }
        }

        // Ensure memory growth is bounded across all subnets
        let mut empty_subnets = Vec::new();
        for (sub, evts) in self.history.iter_mut() {
            if sub == &subnet { continue; } // Already pruned
            while let Some(evt) = evts.front() {
                if evt.timestamp < window_start {
                    evts.pop_front();
                } else {
                    break;
                }
            }
            if evts.is_empty() {
                empty_subnets.push(*sub);
            }
        }
        for sub in empty_subnets {
            self.history.remove(&sub);
        }

        // Count unique IPs in the current subnet
        let mut unique_ips = HashSet::new();
        for evt in self.history.get(&subnet).unwrap() {
            unique_ips.insert(evt.ip);
        }

        if unique_ips.len() >= self.threshold {
            // Remove the subnet from history to prevent re-escalation on the same IPs
            self.history.remove(&subnet);
            EscalationResult::Escalated {
                subnet,
                duration: original_duration * self.escalation_multiplier,
            }
        } else {
            EscalationResult::NotEscalated
        }
    }
}
