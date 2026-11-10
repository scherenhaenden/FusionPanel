#![forbid(unsafe_code)]

use monitoring::psi_parser::{PsiParser, PsiData, PsiMetrics, PressureLevel, PressureTrend};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_parse_psi_memory() {
    let mut file = NamedTempFile::new().expect("failed to create temp file");
    let content = "\
some avg10=0.00 avg60=0.00 avg300=0.00 total=4139820
full avg10=0.00 avg60=0.00 avg300=0.00 total=1355403
";
    file.write_all(content.as_bytes()).expect("failed to write");
    let path = file.path();

    let data = PsiParser::parse_file(path).expect("failed to parse");

    assert_eq!(
        data,
        PsiData {
            some: PsiMetrics {
                avg10: 0.0,
                avg60: 0.0,
                avg300: 0.0,
                total: 4139820,
            },
            full: Some(PsiMetrics {
                avg10: 0.0,
                avg60: 0.0,
                avg300: 0.0,
                total: 1355403,
            }),
        }
    );
}

#[test]
fn test_parse_psi_cpu_no_full() {
    let mut file = NamedTempFile::new().expect("failed to create temp file");
    let content = "\
some avg10=2.30 avg60=1.15 avg300=0.50 total=998877
";
    file.write_all(content.as_bytes()).expect("failed to write");
    let path = file.path();

    let data = PsiParser::parse_file(path).expect("failed to parse");

    assert_eq!(
        data,
        PsiData {
            some: PsiMetrics {
                avg10: 2.30,
                avg60: 1.15,
                avg300: 0.50,
                total: 998877,
            },
            full: None,
        }
    );
}

#[test]
fn test_parse_malformed_missing_some() {
    let mut file = NamedTempFile::new().expect("failed to create temp file");
    let content = "full avg10=1.00 avg60=1.00 avg300=1.00 total=123\n";
    file.write_all(content.as_bytes()).expect("failed to write");
    let path = file.path();

    let res = PsiParser::parse_file(path);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "Missing 'some' metrics in PSI data");
}

#[test]
fn test_calculate_level() {
    let degraded = 10.0;
    let critical = 20.0;

    let normal_metric = PsiMetrics { avg10: 5.0, avg60: 5.0, avg300: 5.0, total: 100 };
    assert_eq!(PsiParser::calculate_level(&normal_metric, degraded, critical), PressureLevel::Normal);

    let degraded_metric = PsiMetrics { avg10: 15.0, avg60: 5.0, avg300: 5.0, total: 100 };
    assert_eq!(PsiParser::calculate_level(&degraded_metric, degraded, critical), PressureLevel::Degraded);

    let critical_metric = PsiMetrics { avg10: 25.0, avg60: 5.0, avg300: 5.0, total: 100 };
    assert_eq!(PsiParser::calculate_level(&critical_metric, degraded, critical), PressureLevel::Critical);
}

#[test]
fn test_calculate_trend() {
    let increasing = PsiMetrics { avg10: 10.0, avg60: 5.0, avg300: 2.0, total: 100 };
    assert_eq!(PsiParser::calculate_trend(&increasing), PressureTrend::Increasing);

    let decreasing = PsiMetrics { avg10: 2.0, avg60: 8.0, avg300: 10.0, total: 100 };
    assert_eq!(PsiParser::calculate_trend(&decreasing), PressureTrend::Decreasing);

    let stable = PsiMetrics { avg10: 5.2, avg60: 5.0, avg300: 5.1, total: 100 };
    assert_eq!(PsiParser::calculate_trend(&stable), PressureTrend::Stable);
}
