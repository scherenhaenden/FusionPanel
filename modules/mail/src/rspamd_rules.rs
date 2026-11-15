#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct RspamdThresholds {
    pub reject: Option<f64>,
    pub add_header: Option<f64>,
    pub rewrite_subject: Option<f64>,
    pub greylist: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RspamdRuleFormatter {
    pub thresholds: RspamdThresholds,
    pub symbol_weights: BTreeMap<String, f64>,
}

impl Default for RspamdThresholds {
    fn default() -> Self {
        Self {
            reject: Some(15.0),
            add_header: Some(6.0),
            rewrite_subject: Some(6.0),
            greylist: Some(4.0),
        }
    }
}

impl RspamdRuleFormatter {
    pub fn new() -> Self {
        Self {
            thresholds: RspamdThresholds::default(),
            symbol_weights: BTreeMap::new(),
        }
    }

    pub fn with_thresholds(mut self, thresholds: RspamdThresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    pub fn with_symbol_weight(mut self, symbol: impl Into<String>, weight: f64) -> Self {
        self.symbol_weights.insert(symbol.into(), weight);
        self
    }

    pub fn generate_actions_conf(&self) -> String {
        let mut out = String::new();
        out.push_str("actions {\n");

        if let Some(r) = self.thresholds.reject {
            let _ = writeln!(out, "    reject = {};", r);
        }
        if let Some(a) = self.thresholds.add_header {
            let _ = writeln!(out, "    add_header = {};", a);
        }
        if let Some(rs) = self.thresholds.rewrite_subject {
            let _ = writeln!(out, "    rewrite_subject = {};", rs);
        }
        if let Some(g) = self.thresholds.greylist {
            let _ = writeln!(out, "    greylist = {};", g);
        }

        out.push_str("}\n");
        out
    }

    pub fn generate_metrics_conf(&self) -> String {
        if self.symbol_weights.is_empty() {
            return String::new();
        }

        let mut out = String::new();
        out.push_str("group \"default\" {\n");

        for (symbol, weight) in &self.symbol_weights {
            let _ = writeln!(out, "    symbol \"{}\" {{", symbol);
            let _ = writeln!(out, "        weight = {:.2};", weight);
            out.push_str("    }\n");
        }

        out.push_str("}\n");
        out
    }
}

impl Default for RspamdRuleFormatter {
    fn default() -> Self {
        Self::new()
    }
}
