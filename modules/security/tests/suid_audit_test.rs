#![forbid(unsafe_code)]

use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;
use security::{RiskLevel, SuidScanner, SuidFinding};

#[test]
fn test_suid_permission_evaluation() {
    let dir = tempdir().unwrap();
    let scanner = SuidScanner::new();

    let suid_path = dir.path().join("my_suid_bin");
    File::create(&suid_path).unwrap();
    fs::set_permissions(&suid_path, fs::Permissions::from_mode(0o4755)).unwrap();

    let sgid_path = dir.path().join("my_sgid_bin");
    File::create(&sgid_path).unwrap();
    fs::set_permissions(&sgid_path, fs::Permissions::from_mode(0o2755)).unwrap();

    let normal_path = dir.path().join("my_normal_bin");
    File::create(&normal_path).unwrap();
    fs::set_permissions(&normal_path, fs::Permissions::from_mode(0o0755)).unwrap();

    let findings = scanner.scan_path(dir.path()).unwrap();

    assert_eq!(findings.len(), 2);

    let suid_finding = findings.iter().find(|f| f.path == suid_path).unwrap();
    assert!(suid_finding.is_suid);
    assert!(!suid_finding.is_sgid);
    assert_eq!(suid_finding.risk_level, RiskLevel::Suspicious);

    let sgid_finding = findings.iter().find(|f| f.path == sgid_path).unwrap();
    assert!(!sgid_finding.is_suid);
    assert!(sgid_finding.is_sgid);
    assert_eq!(sgid_finding.risk_level, RiskLevel::Suspicious);
}

#[test]
fn test_gtfobins_classification() {
    let dir = tempdir().unwrap();
    let scanner = SuidScanner::new();

    let bash_path = dir.path().join("bash");
    File::create(&bash_path).unwrap();
    fs::set_permissions(&bash_path, fs::Permissions::from_mode(0o4755)).unwrap();

    let sudo_path = dir.path().join("sudo");
    File::create(&sudo_path).unwrap();
    fs::set_permissions(&sudo_path, fs::Permissions::from_mode(0o4755)).unwrap();

    let findings = scanner.scan_path(dir.path()).unwrap();

    assert_eq!(findings.len(), 2);

    let bash_finding = findings.iter().find(|f| f.path == bash_path).unwrap();
    assert_eq!(bash_finding.risk_level, RiskLevel::Critical);

    let sudo_finding = findings.iter().find(|f| f.path == sudo_path).unwrap();
    assert_eq!(sudo_finding.risk_level, RiskLevel::Standard);
}

#[test]
fn test_baseline_comparison() {
    let scanner = SuidScanner::new();

    let baseline = vec![
        SuidFinding {
            path: "/bin/sudo".into(),
            risk_level: RiskLevel::Standard,
            is_suid: true,
            is_sgid: false,
        },
    ];

    let current = vec![
        SuidFinding {
            path: "/bin/sudo".into(),
            risk_level: RiskLevel::Standard,
            is_suid: true,
            is_sgid: false,
        },
        SuidFinding {
            path: "/tmp/bash".into(),
            risk_level: RiskLevel::Critical,
            is_suid: true,
            is_sgid: false,
        },
    ];

    let new_findings = scanner.compare_baseline(&baseline, &current);

    assert_eq!(new_findings.len(), 1);
    assert_eq!(new_findings[0].path.to_str().unwrap(), "/tmp/bash");
    assert_eq!(new_findings[0].risk_level, RiskLevel::Critical);
}
