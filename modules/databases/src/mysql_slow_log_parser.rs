#![forbid(unsafe_code)]

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use thiserror::Error;

static IN_CLAUSE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bIN\s*\([^)]+\)").unwrap());

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Failed to parse log line: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlowQueryEntry {
    pub timestamp: Option<String>,
    pub user_host: String,
    pub query_time: f64,
    pub lock_time: f64,
    pub rows_sent: u64,
    pub rows_examined: u64,
    pub query: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryStats {
    pub fingerprint: String,
    pub total_calls: u64,
    pub total_time: f64,
    pub latencies: Vec<f64>,
}

impl QueryStats {
    pub fn new(fingerprint: String) -> Self {
        Self {
            fingerprint,
            total_calls: 0,
            total_time: 0.0,
            latencies: Vec::new(),
        }
    }

    pub fn add_call(&mut self, query_time: f64) {
        self.total_calls += 1;
        self.total_time += query_time;
        self.latencies.push(query_time);
    }

    pub fn p95_latency(&self) -> f64 {
        if self.latencies.is_empty() {
            return 0.0;
        }
        let mut sorted = self.latencies.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let index = ((self.latencies.len() as f64) * 0.95).ceil() as usize;
        let index = index.saturating_sub(1).min(self.latencies.len() - 1);
        sorted[index]
    }
}

pub struct SlowQueryLogParser;

impl Default for SlowQueryLogParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SlowQueryLogParser {
    pub fn new() -> Self {
        Self
    }

    pub fn fingerprint(query: &str) -> String {
        let query = query.to_string();

        let mut result = String::with_capacity(query.len());
        let chars: Vec<char> = query.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];

            if c == '\'' || c == '"' {
                let quote = c;
                result.push('?');
                i += 1;
                while i < chars.len() {
                    if chars[i] == '\\' {
                        i += 2;
                        continue;
                    }
                    if chars[i] == quote {
                        if i + 1 < chars.len() && chars[i+1] == quote {
                            // SQL double quote escape
                            i += 2;
                            continue;
                        } else {
                            i += 1;
                            break;
                        }
                    }
                    i += 1;
                }
            } else if c.is_ascii_digit() || (c == '-' && i + 1 < chars.len() && chars[i+1].is_ascii_digit()) {
                let mut is_num = false;
                if i == 0 || !chars[i - 1].is_ascii_alphanumeric() && chars[i - 1] != '_' {
                    is_num = true;
                }

                if is_num {
                    result.push('?');
                    if c == '-' {
                        i += 1; // skip '-'
                    }
                    while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                        i += 1;
                    }
                    continue;
                } else {
                    result.push(c);
                    i += 1;
                }
            } else {
                result.push(c);
                i += 1;
            }
        }

        let in_replaced = IN_CLAUSE_REGEX.replace_all(&result, "IN (?)").to_string();

        let words: Vec<&str> = in_replaced.split_whitespace().collect();
        words.join(" ")
    }

    pub fn parse(&self, log: &str) -> Result<Vec<SlowQueryEntry>, ParserError> {
        let mut entries = Vec::new();
        let lines: Vec<&str> = log.lines().collect();
        let mut current_entry: Option<SlowQueryEntry> = None;
        let mut current_query = String::new();
        let mut current_timestamp: Option<String> = None;

        for line in lines {
            if line.starts_with("# Time: ") {
                if let Some(mut entry) = current_entry.take() {
                    entry.query = current_query.trim().to_string();
                    if !entry.query.is_empty() {
                        entries.push(entry);
                    }
                }
                current_query.clear();

                current_timestamp = Some(line["# Time: ".len()..].trim().to_string());
            } else if line.starts_with("# User@Host: ") {
                if let Some(mut entry) = current_entry.take() {
                    entry.query = current_query.trim().to_string();
                    if !entry.query.is_empty() {
                        entries.push(entry);
                    }
                }
                current_query.clear();

                let user_host = line["# User@Host: ".len()..].trim().to_string();
                current_entry = Some(SlowQueryEntry {
                    timestamp: current_timestamp.clone(),
                    user_host,
                    query_time: 0.0,
                    lock_time: 0.0,
                    rows_sent: 0,
                    rows_examined: 0,
                    query: String::new(),
                });
            } else if line.starts_with("# Query_time: ") {
                if let Some(entry) = &mut current_entry {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    for j in 0..parts.len() {
                        match parts[j] {
                            "Query_time:" => {
                                if j + 1 < parts.len() {
                                    entry.query_time = parts[j+1].parse().unwrap_or(0.0);
                                }
                            }
                            "Lock_time:" => {
                                if j + 1 < parts.len() {
                                    entry.lock_time = parts[j+1].parse().unwrap_or(0.0);
                                }
                            }
                            "Rows_sent:" => {
                                if j + 1 < parts.len() {
                                    entry.rows_sent = parts[j+1].parse().unwrap_or(0);
                                }
                            }
                            "Rows_examined:" => {
                                if j + 1 < parts.len() {
                                    entry.rows_examined = parts[j+1].parse().unwrap_or(0);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            } else if !line.starts_with('#') {
                if line.trim().starts_with("SET timestamp=") {
                    let ts_part = line.trim()["SET timestamp=".len()..].trim();
                    let ts_str = ts_part.trim_end_matches(';').to_string();
                    if let Some(entry) = &mut current_entry {
                        entry.timestamp = Some(ts_str.clone());
                    }
                    current_timestamp = Some(ts_str);
                } else if line.trim().starts_with("use ") {
                    // ignore use db
                } else {
                    current_query.push_str(line);
                    current_query.push('\n');
                }
            }
        }

        if let Some(mut entry) = current_entry.take() {
            entry.query = current_query.trim().to_string();
            if !entry.query.is_empty() {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    pub fn aggregate(&self, entries: &[SlowQueryEntry]) -> Vec<QueryStats> {
        let mut stats_map: HashMap<String, QueryStats> = HashMap::new();

        for entry in entries {
            let fingerprint = Self::fingerprint(&entry.query);
            let stats = stats_map.entry(fingerprint.clone()).or_insert_with(|| QueryStats::new(fingerprint));
            stats.add_call(entry.query_time);
        }

        let mut stats_vec: Vec<QueryStats> = stats_map.into_values().collect();
        stats_vec.sort_by(|a, b| b.total_time.partial_cmp(&a.total_time).unwrap_or(std::cmp::Ordering::Equal));
        stats_vec
    }
}
