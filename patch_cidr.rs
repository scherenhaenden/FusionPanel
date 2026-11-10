<<<<<<< SEARCH
    pub fn contains(&self, other_ip: IpAddr) -> bool {
        match (self.ip, other_ip) {
            (IpAddr::V4(a), IpAddr::V4(b)) => {
                if self.prefix >= 32 {
                    return a == b;
                }
                let mask = u32::MAX << (32 - self.prefix);
                let a_u32 = u32::from_be_bytes(a.octets());
                let b_u32 = u32::from_be_bytes(b.octets());
                (a_u32 & mask) == (b_u32 & mask)
            }
            (IpAddr::V6(a), IpAddr::V6(b)) => {
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
=======
    pub fn contains(&self, other_ip: IpAddr) -> bool {
        match (self.ip, other_ip) {
            (IpAddr::V4(a), IpAddr::V4(b)) => {
                if self.prefix == 0 {
                    return true;
                }
                if self.prefix >= 32 {
                    return a == b;
                }
                let mask = u32::MAX << (32 - self.prefix);
                let a_u32 = u32::from_be_bytes(a.octets());
                let b_u32 = u32::from_be_bytes(b.octets());
                (a_u32 & mask) == (b_u32 & mask)
            }
            (IpAddr::V6(a), IpAddr::V6(b)) => {
                if self.prefix == 0 {
                    return true;
                }
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
                if min_prefix == 0 {
                    return true;
                }
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
                if min_prefix == 0 {
                    return true;
                }
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
>>>>>>> REPLACE
