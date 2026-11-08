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

    pub fn audit(
        &self,
        spf: Option<&str>,
        dkim: &[&str],
        dmarc: Option<&str>,
    ) -> AuditReport {
        let mut score = 0;
        let mut weaknesses = Vec::new();
        let mut recommendations = Vec::new();

        // SPF Evaluation (Max 30)
        if let Some(spf_rec) = spf {
            score += 20;
            if spf_rec.contains("-all") {
                score += 10;
            } else if spf_rec.contains("~all") {
                score += 5;
                weaknesses.push("SPF uses softfail (~all)".to_string());
                recommendations.push("Upgrade SPF to strict fail (-all).".to_string());
            } else if spf_rec.contains("?all") || spf_rec.contains("+all") {
                score -= 10;
                weaknesses.push("SPF is too permissive".to_string());
                recommendations.push("Change SPF to -all.".to_string());
            } else {
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
            let mut has_testing = false;
            for rec in dkim {
                if rec.contains("t=y") {
                    has_testing = true;
                }
            }
            if has_testing {
                weaknesses.push("DKIM in testing mode (t=y)".to_string());
                recommendations.push("Remove t=y from DKIM record.".to_string());
            } else {
                score += 10;
            }
        }

        // DMARC Evaluation (Max 40)
        if let Some(dmarc_rec) = dmarc {
            score += 10;

            if dmarc_rec.contains("p=reject") {
                score += 20;
            } else if dmarc_rec.contains("p=quarantine") {
                score += 10;
                weaknesses.push("DMARC policy is quarantine, not reject".to_string());
                recommendations.push("Upgrade DMARC policy to p=reject.".to_string());
            } else if dmarc_rec.contains("p=none") {
                score += 0;
                weaknesses.push("DMARC policy is none".to_string());
                recommendations.push("Upgrade DMARC policy to p=quarantine or p=reject.".to_string());
            } else {
                weaknesses.push("DMARC policy is missing or unrecognized".to_string());
                recommendations.push("Set a valid DMARC policy (p=reject, p=quarantine, or p=none).".to_string());
            }

            if dmarc_rec.contains("rua=") {
                score += 5;
            } else {
                weaknesses.push("DMARC missing rua".to_string());
                recommendations.push("Add rua=mailto:... to DMARC for aggregate reports.".to_string());
            }

            if dmarc_rec.contains("ruf=") {
                score += 5;
            } else {
                weaknesses.push("DMARC missing ruf".to_string());
                recommendations.push("Add ruf=mailto:... to DMARC for forensic reports.".to_string());
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
