#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grade {
    APlus,
    A,
    B,
    C,
    F,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditReport {
    pub grade: Grade,
    pub score: i32,
    pub weaknesses: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Default)]
pub struct DomainSecurityAuditor;

impl DomainSecurityAuditor {
    pub fn new() -> Self {
        Self
    }

    pub fn audit(&self, spf: Option<&str>, dkim: &[&str], dmarc: Option<&str>) -> AuditReport {
        let mut score = 0;
        let mut weaknesses = Vec::new();
        let mut recommendations = Vec::new();

        // SPF Evaluation (Max 30)
        if let Some(spf_rec) = spf {
            let mechanisms = spf_mechanisms(spf_rec);
            if mechanisms.is_empty() {
                weaknesses.push("Invalid SPF record".to_string());
                recommendations
                    .push("Publish a valid SPF record beginning with v=spf1.".to_string());
            } else if mechanisms.iter().any(|term| term == "-all") {
                score += 30;
            } else if mechanisms.iter().any(|term| term == "~all") {
                score += 25;
                weaknesses.push("SPF uses softfail (~all)".to_string());
                recommendations.push("Upgrade SPF to strict fail (-all).".to_string());
            } else if mechanisms
                .iter()
                .any(|term| term == "?all" || term == "+all" || term == "all")
            {
                score += 10;
                weaknesses.push("SPF is too permissive".to_string());
                recommendations.push("Change SPF to -all.".to_string());
            } else {
                score += 20;
                weaknesses.push("SPF does not specify an 'all' mechanism".to_string());
                recommendations.push("Add -all to the end of your SPF record.".to_string());
            }
        } else {
            weaknesses.push("Missing SPF record".to_string());
            recommendations.push("Configure an SPF record.".to_string());
        }

        // DKIM Evaluation (Max 30)
        if dkim.is_empty() {
            weaknesses.push("Missing DKIM record".to_string());
            recommendations.push("Configure DKIM for email signing.".to_string());
        } else {
            score += 20;
            let has_testing = dkim.iter().any(|record| {
                record.split(';').any(|tag| {
                    let (name, value) = tag.trim().split_once('=').unwrap_or(("", ""));
                    name.trim().eq_ignore_ascii_case("t")
                        && value
                            .split(':')
                            .any(|flag| flag.trim().eq_ignore_ascii_case("y"))
                })
            });
            if has_testing {
                weaknesses.push("DKIM in testing mode (t=y)".to_string());
                recommendations.push("Remove t=y from DKIM record.".to_string());
            } else {
                score += 10;
            }
        }

        // DMARC Evaluation (Max 40)
        if let Some(dmarc_rec) = dmarc {
            let tags = dmarc_tags(dmarc_rec);
            let policy = tags.get("p").map(String::as_str);
            if !dmarc_rec
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("v=dmarc1;")
            {
                weaknesses.push("Invalid DMARC record".to_string());
                recommendations
                    .push("Publish a valid DMARC record beginning with v=DMARC1.".to_string());
            } else {
                score += 10;
            }

            if policy == Some("reject") {
                score += 20;
            } else if policy == Some("quarantine") {
                score += 10;
                weaknesses.push("DMARC policy is quarantine, not reject".to_string());
                recommendations.push("Upgrade DMARC policy to p=reject.".to_string());
            } else if policy == Some("none") {
                score += 10;
                weaknesses.push("DMARC policy is none".to_string());
                recommendations
                    .push("Upgrade DMARC policy to p=quarantine or p=reject.".to_string());
            } else {
                weaknesses.push("DMARC policy is missing or unrecognized".to_string());
                recommendations.push(
                    "Set a valid DMARC policy (p=reject, p=quarantine, or p=none).".to_string(),
                );
            }

            if tags.contains_key("rua") {
                score += 5;
            } else {
                weaknesses.push("DMARC missing rua".to_string());
                recommendations
                    .push("Add rua=mailto:... to DMARC for aggregate reports.".to_string());
            }

            if tags.contains_key("ruf") {
                score += 5;
            } else {
                weaknesses.push("DMARC missing ruf".to_string());
                recommendations
                    .push("Add ruf=mailto:... to DMARC for forensic reports.".to_string());
            }
        } else {
            weaknesses.push("Missing DMARC record".to_string());
            recommendations.push("Configure a DMARC record.".to_string());
        }

        let grade = if score >= 100 {
            Grade::APlus
        } else if score >= 90 {
            Grade::A
        } else if score >= 75 {
            Grade::B
        } else if score >= 35 {
            Grade::C
        } else {
            Grade::F
        };

        AuditReport {
            grade,
            score,
            weaknesses,
            recommendations,
        }
    }
}

fn spf_mechanisms(record: &str) -> Vec<String> {
    let mut terms = record.split_whitespace();
    if !terms
        .next()
        .is_some_and(|version| version.eq_ignore_ascii_case("v=spf1"))
    {
        return Vec::new();
    }

    terms.map(str::to_ascii_lowercase).collect()
}

fn dmarc_tags(record: &str) -> std::collections::HashMap<String, String> {
    record
        .split(';')
        .filter_map(|tag| tag.trim().split_once('='))
        .map(|(name, value)| {
            (
                name.trim().to_ascii_lowercase(),
                value.trim().to_ascii_lowercase(),
            )
        })
        .collect()
}

#[cfg(test)]
mod regression_tests {
    use super::{DomainSecurityAuditor, Grade};

    #[test]
    fn embedded_spf_substring_does_not_count_as_fail_qualifier() {
        let report =
            DomainSecurityAuditor.audit(Some("v=spf1 include:example.test -all.evil"), &[], None);

        assert_ne!(report.grade, Grade::APlus);
        assert!(report
            .weaknesses
            .contains(&"SPF does not specify an 'all' mechanism".to_string()));
    }

    #[test]
    fn embedded_dmarc_substring_does_not_count_as_reject_policy() {
        let report = DomainSecurityAuditor.audit(
            Some("v=spf1 -all"),
            &[],
            Some("v=DMARC1; x=p=reject; rua=mailto:reports@example.test"),
        );

        assert!(report
            .weaknesses
            .contains(&"DMARC policy is missing or unrecognized".to_string()));
    }

    #[test]
    fn dkim_tag_substring_does_not_count_as_testing_mode() {
        let report = DomainSecurityAuditor.audit(
            Some("v=spf1 -all"),
            &["v=DKIM1; x=t=y;"],
            Some("v=DMARC1; p=reject; rua=mailto:reports@example.test; ruf=mailto:forensics@example.test"),
        );

        assert!(!report
            .weaknesses
            .iter()
            .any(|weakness| weakness.contains("testing mode")));
    }
}
