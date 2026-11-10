#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fmt;
use std::net::Ipv4Addr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IpamError {
    #[error("IPAM pool exhausted: no available subnets")]
    PoolExhausted,
    #[error("Invalid CIDR format: {0}")]
    InvalidCidr(String),
    #[error("Allocation size {0} is larger than pool size {1}")]
    InvalidAllocationSize(u8, u8),
    #[error("Not an IPv4 CIDR")]
    NotIpv4,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ipv4Cidr {
    pub ip: u32,
    pub prefix: u8,
}

impl Ipv4Cidr {
    pub fn new(ip: Ipv4Addr, prefix: u8) -> Result<Self, IpamError> {
        if prefix > 32 {
            return Err(IpamError::InvalidCidr(format!("Invalid prefix: {}", prefix)));
        }

        let ip_u32 = u32::from(ip);
        let mask = Self::mask(prefix);

        // Ensure the IP is the network address (no host bits set)
        Ok(Self {
            ip: ip_u32 & mask,
            prefix,
        })
    }

    pub fn parse(s: &str) -> Result<Self, IpamError> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            return Err(IpamError::InvalidCidr("Missing slash".into()));
        }

        let ip_str = parts[0];
        let prefix_str = parts[1];

        let ip: Ipv4Addr = ip_str.parse().map_err(|_| IpamError::InvalidCidr("Invalid IP".into()))?;
        let prefix: u8 = prefix_str.parse().map_err(|_| IpamError::InvalidCidr("Invalid prefix format".into()))?;

        Self::new(ip, prefix)
    }

    fn mask(prefix: u8) -> u32 {
        if prefix == 0 {
            0
        } else {
            (!0u32) << (32 - prefix)
        }
    }

    pub fn overlaps_with(&self, other: &Self) -> bool {
        let min_prefix = std::cmp::min(self.prefix, other.prefix);
        let mask = Self::mask(min_prefix);
        (self.ip & mask) == (other.ip & mask)
    }

    fn to_string_internal(self) -> String {
        let ip = Ipv4Addr::from(self.ip);
        format!("{}/{}", ip, self.prefix)
    }
}

impl fmt::Debug for Ipv4Cidr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_internal())
    }
}

impl fmt::Display for Ipv4Cidr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_internal())
    }
}

pub struct IpamAllocator {
    pool: Ipv4Cidr,
    alloc_prefix: u8,
    allocated: HashSet<Ipv4Cidr>,
    reserved: HashSet<Ipv4Cidr>,
}

impl IpamAllocator {
    pub fn new(pool_cidr: &str, alloc_prefix: u8) -> Result<Self, IpamError> {
        let pool = Ipv4Cidr::parse(pool_cidr)?;

        if alloc_prefix < pool.prefix {
            return Err(IpamError::InvalidAllocationSize(alloc_prefix, pool.prefix));
        }

        if alloc_prefix > 32 {
            return Err(IpamError::InvalidCidr(format!("Invalid allocation prefix: {}", alloc_prefix)));
        }

        Ok(Self {
            pool,
            alloc_prefix,
            allocated: HashSet::new(),
            reserved: HashSet::new(),
        })
    }

    pub fn reserve(&mut self, cidr: &str) -> Result<(), IpamError> {
        let r = Ipv4Cidr::parse(cidr)?;
        self.reserved.insert(r);
        Ok(())
    }

    pub fn allocate(&mut self) -> Result<String, IpamError> {
        // Compute how many subnets we can have
        let shift_amount = self.alloc_prefix - self.pool.prefix;
        if shift_amount >= 32 {
             // Extremely large pool vs alloc gap, unlikely but we handle it safely
             return Err(IpamError::PoolExhausted);
        }
        let total_subnets: u32 = 1u32.checked_shl(shift_amount as u32).unwrap_or(u32::MAX);

        let alloc_size: u32 = if self.alloc_prefix == 32 {
            1
        } else {
            1u32 << (32 - self.alloc_prefix)
        };

        for i in 0..total_subnets {
            // calculate the candidate ip
            let candidate_ip = self.pool.ip + (i * alloc_size);
            let candidate = Ipv4Cidr {
                ip: candidate_ip,
                prefix: self.alloc_prefix,
            };

            // check collisions
            let mut collides = false;
            for r in &self.reserved {
                if candidate.overlaps_with(r) {
                    collides = true;
                    break;
                }
            }

            if !collides {
                for a in &self.allocated {
                    if candidate.overlaps_with(a) {
                        collides = true;
                        break;
                    }
                }
            }

            if !collides {
                self.allocated.insert(candidate);
                return Ok(candidate.to_string());
            }
        }

        Err(IpamError::PoolExhausted)
    }

    pub fn free(&mut self, cidr: &str) -> Result<(), IpamError> {
        let c = Ipv4Cidr::parse(cidr)?;
        self.allocated.remove(&c);
        Ok(())
    }
}
