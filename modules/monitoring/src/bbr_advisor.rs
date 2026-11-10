#![forbid(unsafe_code)]

/// Represents network link characteristics for auto-tuning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NetworkLink {
    /// Bandwidth in Megabits per second (Mbps)
    pub bandwidth_mbps: f64,
    /// Round Trip Time in milliseconds (ms)
    pub rtt_ms: f64,
}

impl NetworkLink {
    /// Creates a new `NetworkLink`. Ensures values are non-negative.
    pub fn new(bandwidth_mbps: f64, rtt_ms: f64) -> Self {
        Self {
            bandwidth_mbps: bandwidth_mbps.max(0.0),
            rtt_ms: rtt_ms.max(0.0),
        }
    }

    /// Computes the Bandwidth-Delay Product (BDP) in bytes.
    pub fn bdp_bytes(&self) -> u64 {
        // Bandwidth in bits per second
        let bps = self.bandwidth_mbps * 1_000_000.0;
        // Bandwidth in bytes per second
        let bytes_per_sec = bps / 8.0;
        // RTT in seconds
        let rtt_sec = self.rtt_ms / 1000.0;

        (bytes_per_sec * rtt_sec).round() as u64
    }
}

/// Linux kernel sysctl parameters recommended for BBR and socket buffers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BbrSysctlParams {
    pub tcp_congestion_control: String,
    pub default_qdisc: String,
    pub tcp_rmem: String,
    pub tcp_wmem: String,
}

pub struct BbrAdvisor;

impl BbrAdvisor {
    /// Calculates optimal sysctl parameters for the given network link.
    pub fn advise(link: &NetworkLink) -> BbrSysctlParams {
        let bdp = link.bdp_bytes();

        // Standard minimum socket buffer size: 4096 bytes (4 KB)
        let min_buf = 4096;

        // Default buffer size: at least 65536 bytes (64 KB)
        let default_buf = std::cmp::max(65536, bdp / 2);

        // Maximum buffer size: at least 4194304 bytes (4 MB)
        // Scaled to 2x BDP to allow window scaling and handle fluctuations
        let max_buf = std::cmp::max(4_194_304, bdp.saturating_mul(2));

        let rmem = format!("{} {} {}", min_buf, default_buf, max_buf);
        let wmem = format!("{} {} {}", min_buf, default_buf, max_buf);

        BbrSysctlParams {
            tcp_congestion_control: "bbr".to_string(),
            default_qdisc: "fq".to_string(),
            tcp_rmem: rmem,
            tcp_wmem: wmem,
        }
    }
}
