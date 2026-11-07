#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DmarcPolicy {
    None,
    Quarantine,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Alignment {
    Strict,
    Relaxed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthResult {
    Pass,
    Fail,
    Neutral,
    SoftFail,
    TempError,
    PermError,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpfStatus {
    pub result: AuthResult,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkimStatus {
    pub result: AuthResult,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmarcConfig {
    pub policy: DmarcPolicy,
    pub spf_alignment: Alignment,
    pub dkim_alignment: Alignment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DmarcResult {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmarcEvaluation {
    pub result: DmarcResult,
    pub applied_policy: DmarcPolicy,
    pub spf_aligned: bool,
    pub dkim_aligned: bool,
}

pub fn is_subdomain_or_exact(child: &str, parent: &str) -> bool {
    if child.eq_ignore_ascii_case(parent) {
        return true;
    }
    if child.len() > parent.len() {
        let prefix_len = child.len() - parent.len();
        if child.as_bytes()[prefix_len - 1] == b'.'
            && child[prefix_len..].eq_ignore_ascii_case(parent)
        {
            return true;
        }
    }
    false
}

pub fn check_alignment(header_from: &str, auth_domain: &str, alignment: &Alignment) -> bool {
    match alignment {
        Alignment::Strict => header_from.eq_ignore_ascii_case(auth_domain),
        Alignment::Relaxed => {
            // Simplified relaxed alignment: auth_domain must be a suffix (parent) of header_from or exact match
            // In a real implementation we would parse organizational domains (e.g., using Public Suffix List).
            // For this evaluator, we assume auth_domain is the organizational domain or we check subdomain.
            is_subdomain_or_exact(header_from, auth_domain)
        }
    }
}

pub fn evaluate_dmarc(
    header_from: &str,
    spf: &SpfStatus,
    dkim: &DkimStatus,
    config: &DmarcConfig,
) -> DmarcEvaluation {
    let spf_aligned = spf.result == AuthResult::Pass
        && check_alignment(header_from, &spf.domain, &config.spf_alignment);

    let dkim_aligned = dkim.result == AuthResult::Pass
        && check_alignment(header_from, &dkim.domain, &config.dkim_alignment);

    let pass = spf_aligned || dkim_aligned;
    let result = if pass {
        DmarcResult::Pass
    } else {
        DmarcResult::Fail
    };
    let applied_policy = if pass {
        DmarcPolicy::None
    } else {
        config.policy.clone()
    };

    DmarcEvaluation {
        result,
        applied_policy,
        spf_aligned,
        dkim_aligned,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateReportRow {
    pub source_ip: String,
    pub count: u32,
    pub evaluation: DmarcEvaluation,
    pub header_from: String,
    pub spf_domain: String,
    pub spf_result: AuthResult,
    pub dkim_domain: String,
    pub dkim_result: AuthResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateReport {
    pub org_name: String,
    pub email: String,
    pub report_id: String,
    pub domain: String,
    pub begin_date: u64,
    pub end_date: u64,
    pub rows: Vec<AggregateReportRow>,
}

impl AggregateReport {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}
