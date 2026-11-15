#![forbid(unsafe_code)]

use monitoring::io_psi::{IoPsiMonitor, IoPressureState};
use tempfile::NamedTempFile;
use std::fs;

#[test]
fn test_parse_psi_normal() {
    let content = "some avg10=0.00 avg60=0.00 avg300=0.00 total=12345\nfull avg10=0.00 avg60=0.00 avg300=0.00 total=0\n";
    let data = IoPsiMonitor::parse_psi(content).unwrap();

    assert_eq!(data.some.avg10, 0.0);
    assert_eq!(data.some.total, 12345);
    assert_eq!(data.full.avg10, 0.0);
    assert_eq!(data.full.total, 0);

    let monitor = IoPsiMonitor::default();
    let report = monitor.analyze(&data);
    assert_eq!(report.state, IoPressureState::Normal);
    assert_eq!(report.is_bottleneck, false);
}

#[test]
fn test_parse_psi_elevated() {
    let content = "some avg10=15.50 avg60=5.00 avg300=2.00 total=12345\nfull avg10=1.00 avg60=0.00 avg300=0.00 total=100\n";
    let data = IoPsiMonitor::parse_psi(content).unwrap();

    let monitor = IoPsiMonitor::default();
    let report = monitor.analyze(&data);
    assert_eq!(report.state, IoPressureState::Elevated);
    assert_eq!(report.is_bottleneck, false);
}

#[test]
fn test_parse_psi_critical_bottleneck() {
    let content = "some avg10=45.00 avg60=20.00 avg300=10.00 total=12345\nfull avg10=15.00 avg60=5.00 avg300=1.00 total=500\n";
    let data = IoPsiMonitor::parse_psi(content).unwrap();

    let monitor = IoPsiMonitor::default();
    let report = monitor.analyze(&data);
    assert_eq!(report.state, IoPressureState::Critical);
    assert_eq!(report.is_bottleneck, true);
}

#[test]
fn test_read_and_parse_valid() {
    let file = NamedTempFile::new().unwrap();
    let content = "some avg10=2.50 avg60=1.00 avg300=0.50 total=12345\nfull avg10=0.50 avg60=0.10 avg300=0.00 total=100\n";
    fs::write(file.path(), content).unwrap();

    let data = IoPsiMonitor::read_and_parse(file.path().to_str().unwrap()).unwrap();
    assert_eq!(data.some.avg10, 2.50);
}

#[test]
fn test_read_and_parse_invalid() {
    let file = NamedTempFile::new().unwrap();
    let content = "invalid contents without some or full";
    fs::write(file.path(), content).unwrap();

    let result = IoPsiMonitor::read_and_parse(file.path().to_str().unwrap());
    assert!(result.is_err());
}
