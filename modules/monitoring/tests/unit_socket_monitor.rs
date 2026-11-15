#![forbid(unsafe_code)]

use monitoring::socket_monitor::{SocketAnomaly, SocketMonitor, SocketStats};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_parse_sockstat() {
    let content = "sockets: used 155\n\
                   TCP: inuse 10 orphan 2 tw 4 alloc 15 mem 1\n\
                   UDP: inuse 5 mem 2\n\
                   UDPLITE: inuse 0\n\
                   RAW: inuse 0\n\
                   FRAG: inuse 0 memory 0\n";
    let stats = SocketMonitor::parse_sockstat(content).expect("Failed to parse sockstat");
    assert_eq!(stats.orphan, 2);
    assert_eq!(stats.time_wait, 4);
    assert_eq!(stats.alloc, 15);
}

#[test]
fn test_parse_tcp() {
    let content = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode                                                     \n\
                   0: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 46083 1 0000000000000000 100 0 0 10 0                     \n\
                   1: 0100007F:0277 00000000:0000 01 00000000:00000000 00:00000000 00000000     0        0 24194 1 0000000000000000 100 0 0 10 0                     \n\
                   2: 00000000:A146 00000000:0000 08 00000000:00000000 00:00000000 00000000  1000        0 27958 1 0000000000000000 100 0 0 10 0                     \n";
    let mut stats = SocketStats::default();
    SocketMonitor::parse_tcp(content, &mut stats);
    assert_eq!(stats.established, 1);
    assert_eq!(stats.close_wait, 1);
}

#[test]
fn test_exhaustion_risk() {
    let mut sockstat = NamedTempFile::new().unwrap();
    let mut tcp = NamedTempFile::new().unwrap();

    let sockstat_content = "TCP: inuse 10 orphan 2 tw 4 alloc 85 mem 1\n";
    let tcp_content = "  sl  local_address rem_address   st \n\
                       0: 00000000:0016 00000000:0000 01 \n";

    sockstat.write_all(sockstat_content.as_bytes()).unwrap();
    tcp.write_all(tcp_content.as_bytes()).unwrap();

    let mut monitor = SocketMonitor::new(100);
    let anomalies = monitor.analyze(sockstat.path(), tcp.path()).unwrap();

    assert_eq!(anomalies.len(), 1);
    assert_eq!(anomalies[0], SocketAnomaly::ExhaustionRisk(0.85));
}

#[test]
fn test_high_churn() {
    let mut sockstat = NamedTempFile::new().unwrap();
    let mut tcp = NamedTempFile::new().unwrap();

    let sockstat_content_1 = "TCP: inuse 10 orphan 2 tw 10 alloc 10 mem 1\n";
    let tcp_content = "  sl  local_address rem_address   st \n";

    sockstat.write_all(sockstat_content_1.as_bytes()).unwrap();
    tcp.write_all(tcp_content.as_bytes()).unwrap();

    let mut monitor = SocketMonitor::new(1000);
    let anomalies_1 = monitor.analyze(sockstat.path(), tcp.path()).unwrap();
    assert_eq!(anomalies_1.len(), 0);

    let mut sockstat_2 = NamedTempFile::new().unwrap();
    let sockstat_content_2 = "TCP: inuse 10 orphan 2 tw 1500 alloc 10 mem 1\n";
    sockstat_2.write_all(sockstat_content_2.as_bytes()).unwrap();

    let anomalies_2 = monitor.analyze(sockstat_2.path(), tcp.path()).unwrap();
    assert_eq!(anomalies_2.len(), 1);
    assert_eq!(anomalies_2[0], SocketAnomaly::HighChurn(1490.0));
}
