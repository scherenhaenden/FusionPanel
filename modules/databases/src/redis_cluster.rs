#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterNode {
    pub id: String,
    pub addr: String,
    pub start_slot: u16,
    pub end_slot: u16,
}

pub struct RedisClusterRouter {
    nodes: Vec<ClusterNode>,
}

impl RedisClusterRouter {
    pub fn new(nodes: Vec<ClusterNode>) -> Self {
        Self { nodes }
    }

    pub fn get_hash_slot(key: &str) -> u16 {
        let mut hash_key = key.as_bytes();

        if let Some(start) = hash_key.iter().position(|&b| b == b'{') {
            if let Some(end) = hash_key[start + 1..].iter().position(|&b| b == b'}') {
                if end > 0 {
                    hash_key = &hash_key[start + 1..start + 1 + end];
                }
            }
        }

        Self::crc16(hash_key) & 0x3FFF
    }

    fn crc16(data: &[u8]) -> u16 {
        let mut crc = 0x0000u16;
        for &byte in data {
            crc ^= (byte as u16) << 8;
            for _ in 0..8 {
                if (crc & 0x8000) != 0 {
                    crc = (crc << 1) ^ 0x1021;
                } else {
                    crc <<= 1;
                }
            }
        }
        crc
    }

    pub fn route(&self, key: &str) -> Option<&ClusterNode> {
        let slot = Self::get_hash_slot(key);
        self.nodes.iter().find(|n| slot >= n.start_slot && slot <= n.end_slot)
    }
}
