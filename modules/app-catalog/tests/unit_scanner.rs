#![forbid(unsafe_code)]

use app_catalog::scanner::{ImageScanner, VulnerabilityReport};

const TRIVY_JSON: &str = r#"{
  "SchemaVersion": 2,
  "ArtifactName": "alpine:3.15",
  "Results": [
    {
      "Target": "alpine:3.15 (alpine 3.15.4)",
      "Vulnerabilities": [
        {
          "VulnerabilityID": "CVE-2022-28391",
          "PkgName": "busybox",
          "InstalledVersion": "1.34.1-r4",
          "FixedVersion": "1.34.1-r5",
          "Severity": "CRITICAL"
        },
        {
          "VulnerabilityID": "CVE-2022-37434",
          "PkgName": "zlib",
          "InstalledVersion": "1.2.12-r1",
          "FixedVersion": "1.2.12-r3",
          "Severity": "MEDIUM"
        }
      ]
    }
  ]
}"#;

const GRYPE_JSON: &str = r#"{
  "matches": [
    {
      "vulnerability": {
        "id": "CVE-2022-28391",
        "severity": "Critical",
        "fix": {
          "versions": [
            "1.34.1-r5"
          ],
          "state": "fixed"
        }
      },
      "artifact": {
        "name": "busybox",
        "version": "1.34.1-r4"
      }
    },
    {
      "vulnerability": {
        "id": "CVE-2022-37434",
        "severity": "Medium",
        "fix": {
          "versions": [
            "1.2.12-r3"
          ],
          "state": "fixed"
        }
      },
      "artifact": {
        "name": "zlib",
        "version": "1.2.12-r1"
      }
    }
  ]
}"#;

#[test]
fn test_parse_trivy_json() {
    let reports = ImageScanner::parse_trivy_json(TRIVY_JSON).unwrap();
    assert_eq!(reports.len(), 2);

    assert_eq!(reports[0].cve_id, "CVE-2022-28391");
    assert_eq!(reports[0].affected_package, "busybox");
    assert_eq!(reports[0].severity, "CRITICAL");
    assert_eq!(reports[0].fixed_version.as_deref(), Some("1.34.1-r5"));

    assert_eq!(reports[1].cve_id, "CVE-2022-37434");
    assert_eq!(reports[1].affected_package, "zlib");
    assert_eq!(reports[1].severity, "MEDIUM");
    assert_eq!(reports[1].fixed_version.as_deref(), Some("1.2.12-r3"));
}

#[test]
fn test_parse_grype_json() {
    let reports = ImageScanner::parse_grype_json(GRYPE_JSON).unwrap();
    assert_eq!(reports.len(), 2);

    assert_eq!(reports[0].cve_id, "CVE-2022-28391");
    assert_eq!(reports[0].affected_package, "busybox");
    assert_eq!(reports[0].severity, "CRITICAL");
    assert_eq!(reports[0].fixed_version.as_deref(), Some("1.34.1-r5"));

    assert_eq!(reports[1].cve_id, "CVE-2022-37434");
    assert_eq!(reports[1].affected_package, "zlib");
    assert_eq!(reports[1].severity, "MEDIUM");
    assert_eq!(reports[1].fixed_version.as_deref(), Some("1.2.12-r3"));
}

#[test]
fn test_gate_deployment_fails_on_critical() {
    let reports = vec![VulnerabilityReport {
        cve_id: "CVE-2022-28391".to_string(),
        severity: "CRITICAL".to_string(),
        affected_package: "busybox".to_string(),
        fixed_version: Some("1.34.1-r5".to_string()),
    }];

    let result = ImageScanner::gate_deployment(&reports);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        "Deployment failed: CRITICAL vulnerability found: CVE-2022-28391 in busybox"
    );
}

#[test]
fn test_gate_deployment_passes_on_non_critical() {
    let reports = vec![
        VulnerabilityReport {
            cve_id: "CVE-2022-37434".to_string(),
            severity: "MEDIUM".to_string(),
            affected_package: "zlib".to_string(),
            fixed_version: Some("1.2.12-r3".to_string()),
        },
        VulnerabilityReport {
            cve_id: "CVE-1234-5678".to_string(),
            severity: "HIGH".to_string(),
            affected_package: "curl".to_string(),
            fixed_version: None,
        },
    ];

    let result = ImageScanner::gate_deployment(&reports);
    assert!(result.is_ok());
}
