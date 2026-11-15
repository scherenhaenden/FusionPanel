#![forbid(unsafe_code)]

use monitoring::{BbrAdvisor, NetworkLink};

#[test]
fn test_network_link_initialization() {
    let link = NetworkLink::new(100.0, 50.0);
    assert_eq!(link.bandwidth_mbps, 100.0);
    assert_eq!(link.rtt_ms, 50.0);

    // Negative values should be clamped to 0.0
    let link_neg = NetworkLink::new(-10.0, -5.0);
    assert_eq!(link_neg.bandwidth_mbps, 0.0);
    assert_eq!(link_neg.rtt_ms, 0.0);
}

#[test]
fn test_bdp_calculation() {
    // 100 Mbps = 100,000,000 bits/s = 12,500,000 bytes/s
    // 50 ms = 0.05 s
    // BDP = 12,500,000 * 0.05 = 625,000 bytes
    let link1 = NetworkLink::new(100.0, 50.0);
    assert_eq!(link1.bdp_bytes(), 625_000);

    // 1000 Mbps (1 Gbps) and 10 ms RTT
    // 1,000,000,000 bps = 125,000,000 bytes/s
    // BDP = 125,000,000 * 0.01 = 1,250,000 bytes
    let link2 = NetworkLink::new(1000.0, 10.0);
    assert_eq!(link2.bdp_bytes(), 1_250_000);

    // 10 Mbps and 200 ms RTT
    // 10,000,000 bps = 1,250,000 bytes/s
    // BDP = 1,250,000 * 0.2 = 250,000 bytes
    let link3 = NetworkLink::new(10.0, 200.0);
    assert_eq!(link3.bdp_bytes(), 250_000);

    // 0 bandwidth or RTT
    let link4 = NetworkLink::new(0.0, 50.0);
    assert_eq!(link4.bdp_bytes(), 0);
    let link5 = NetworkLink::new(100.0, 0.0);
    assert_eq!(link5.bdp_bytes(), 0);
}

#[test]
fn test_bbr_sysctl_advise_standard() {
    // Standard link: 100 Mbps, 50 ms RTT
    // BDP = 625,000 bytes
    // default = max(65536, 625,000 / 2) = 312,500
    // max = max(4,194,304, 625,000 * 2) = 4,194,304
    let link = NetworkLink::new(100.0, 50.0);
    let params = BbrAdvisor::advise(&link);

    assert_eq!(params.tcp_congestion_control, "bbr");
    assert_eq!(params.default_qdisc, "fq");
    assert_eq!(params.tcp_rmem, "4096 312500 4194304");
    assert_eq!(params.tcp_wmem, "4096 312500 4194304");
}

#[test]
fn test_bbr_sysctl_advise_high_bdp() {
    // High BDP link: 10 Gbps, 100 ms RTT
    // 10,000 Mbps = 10,000,000,000 bps = 1,250,000,000 bytes/s
    // BDP = 1,250,000,000 * 0.1 = 125,000,000 bytes
    // default = max(65536, 125,000,000 / 2) = 62,500,000
    // max = max(4,194,304, 125,000,000 * 2) = 250,000,000
    let link = NetworkLink::new(10_000.0, 100.0);
    let params = BbrAdvisor::advise(&link);

    assert_eq!(params.tcp_congestion_control, "bbr");
    assert_eq!(params.default_qdisc, "fq");
    assert_eq!(params.tcp_rmem, "4096 62500000 250000000");
    assert_eq!(params.tcp_wmem, "4096 62500000 250000000");
}

#[test]
fn test_bbr_sysctl_advise_low_bdp() {
    // Low BDP link: 1 Mbps, 10 ms RTT
    // 1 Mbps = 1,000,000 bps = 125,000 bytes/s
    // BDP = 125,000 * 0.01 = 1,250 bytes
    // default = max(65536, 1,250 / 2) = 65536
    // max = max(4,194,304, 1,250 * 2) = 4,194,304
    let link = NetworkLink::new(1.0, 10.0);
    let params = BbrAdvisor::advise(&link);

    assert_eq!(params.tcp_congestion_control, "bbr");
    assert_eq!(params.default_qdisc, "fq");
    assert_eq!(params.tcp_rmem, "4096 65536 4194304");
    assert_eq!(params.tcp_wmem, "4096 65536 4194304");
}

#[test]
fn test_bbr_sysctl_advise_zero_bdp() {
    // Edge case: 0 Mbps or 0 ms RTT
    let link = NetworkLink::new(0.0, 0.0);
    let params = BbrAdvisor::advise(&link);

    assert_eq!(params.tcp_rmem, "4096 65536 4194304");
    assert_eq!(params.tcp_wmem, "4096 65536 4194304");
}
