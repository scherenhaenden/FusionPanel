#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterfaceType {
    Physical,
    DockerBridge,
    Overlay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkInterface {
    pub name: String,
    pub mtu: u32,
    pub interface_type: InterfaceType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtuMismatch {
    pub interface_a: NetworkInterface,
    pub interface_b: NetworkInterface,
}

pub struct MtuClamper;

impl MtuClamper {
    /// Detects MTU differences between host physical interfaces, Docker bridge, and overlay networks.
    pub fn detect_mismatches(interfaces: &[NetworkInterface]) -> Vec<MtuMismatch> {
        let mut mismatches = Vec::new();

        for i in 0..interfaces.len() {
            for j in (i + 1)..interfaces.len() {
                let a = &interfaces[i];
                let b = &interfaces[j];

                // A mismatch is considered when different types of interfaces have different MTUs
                if a.interface_type != b.interface_type && a.mtu != b.mtu {
                    mismatches.push(MtuMismatch {
                        interface_a: a.clone(),
                        interface_b: b.clone(),
                    });
                }
            }
        }

        mismatches
    }

    /// Calculates TCP MSS clamping rules to prevent PMTUD blackholes.
    pub fn calculate_tcp_mss_clamping_rules(interfaces: &[NetworkInterface]) -> Vec<String> {
        let mismatches = Self::detect_mismatches(interfaces);

        if mismatches.is_empty() {
            return vec![];
        }

        vec![
            "iptables -t mangle -A POSTROUTING -p tcp --tcp-flags SYN,RST SYN -j TCPMSS --clamp-mss-to-pmtu".to_string()
        ]
    }
}
