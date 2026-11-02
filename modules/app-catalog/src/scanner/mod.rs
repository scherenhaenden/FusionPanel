#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VulnerabilityReport {
    pub cve_id: String,
    pub severity: String,
    pub affected_package: String,
    pub fixed_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrivyReport {
    #[serde(rename = "Results")]
    results: Option<Vec<TrivyResult>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrivyResult {
    #[serde(rename = "Vulnerabilities")]
    vulnerabilities: Option<Vec<TrivyVulnerability>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrivyVulnerability {
    #[serde(rename = "VulnerabilityID")]
    vulnerability_id: String,
    #[serde(rename = "Severity")]
    severity: String,
    #[serde(rename = "PkgName")]
    pkg_name: String,
    #[serde(rename = "FixedVersion")]
    fixed_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GrypeReport {
    matches: Vec<GrypeMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GrypeMatch {
    vulnerability: GrypeVulnerability,
    artifact: GrypeArtifact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GrypeVulnerability {
    id: String,
    severity: String,
    fix: Option<GrypeFix>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GrypeFix {
    versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GrypeArtifact {
    name: String,
}

pub struct ImageScanner;

impl ImageScanner {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_trivy_json(json: &str) -> Result<Vec<VulnerabilityReport>, serde_json::Error> {
        let report: TrivyReport = serde_json::from_str(json)?;
        let mut vulnerabilities = Vec::new();

        if let Some(results) = report.results {
            for result in results {
                if let Some(vulns) = result.vulnerabilities {
                    for vuln in vulns {
                        vulnerabilities.push(VulnerabilityReport {
                            cve_id: vuln.vulnerability_id,
                            severity: vuln.severity.to_uppercase(),
                            affected_package: vuln.pkg_name,
                            fixed_version: vuln.fixed_version,
                        });
                    }
                }
            }
        }

        Ok(vulnerabilities)
    }

    pub fn parse_grype_json(json: &str) -> Result<Vec<VulnerabilityReport>, serde_json::Error> {
        let report: GrypeReport = serde_json::from_str(json)?;
        let mut vulnerabilities = Vec::new();

        for m in report.matches {
            let fixed_version = m
                .vulnerability
                .fix
                .and_then(|f| f.versions.into_iter().next());
            vulnerabilities.push(VulnerabilityReport {
                cve_id: m.vulnerability.id,
                severity: m.vulnerability.severity.to_uppercase(),
                affected_package: m.artifact.name,
                fixed_version,
            });
        }

        Ok(vulnerabilities)
    }

    pub fn gate_deployment(reports: &[VulnerabilityReport]) -> Result<(), String> {
        for report in reports {
            if report.severity == "CRITICAL" {
                return Err(format!(
                    "Deployment failed: CRITICAL vulnerability found: {} in {}",
                    report.cve_id, report.affected_package
                ));
            }
        }
        Ok(())
    }
}

impl Default for ImageScanner {
    fn default() -> Self {
        Self::new()
    }
}
