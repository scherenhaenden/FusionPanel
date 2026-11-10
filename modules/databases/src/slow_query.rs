#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::io::BufRead;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SlowQueryLogEntry {
    pub timestamp: Option<String>,
    pub user_host: Option<String>,
    pub query_time: f64,
    pub lock_time: f64,
    pub rows_sent: u64,
    pub rows_examined: u64,
    pub sql: String,
}

pub struct SlowQueryLogParser<R> {
    reader: std::io::Lines<R>,
    peeked_line: Option<String>,
    eof: bool,
}

impl<R: BufRead> SlowQueryLogParser<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: reader.lines(),
            peeked_line: None,
            eof: false,
        }
    }

    fn read_line(&mut self) -> std::io::Result<Option<String>> {
        if let Some(line) = self.peeked_line.take() {
            return Ok(Some(line));
        }
        if self.eof {
            return Ok(None);
        }
        match self.reader.next() {
            Some(Ok(line)) => Ok(Some(line)),
            Some(Err(e)) => Err(e),
            None => {
                self.eof = true;
                Ok(None)
            }
        }
    }

    pub fn next_entry(&mut self) -> std::io::Result<Option<SlowQueryLogEntry>> {
        if self.eof && self.peeked_line.is_none() {
            return Ok(None);
        }

        let mut entry = SlowQueryLogEntry::default();
        let mut has_data = false;
        let mut sql_lines = Vec::new();

        while let Some(line) = self.read_line()? {
            if line.starts_with("# Time:") || line.starts_with("# User@Host:") {
                // If we already collected some data and SQL for the current entry,
                // then this header line means a NEW entry is starting.
                if has_data && !sql_lines.is_empty() {
                    self.peeked_line = Some(line);
                    entry.sql = sql_lines.join("\n").trim().to_string();
                    return Ok(Some(entry));
                }
            }

            has_data = true;

            if line.starts_with("# Time:") {
                entry.timestamp = Some(line["# Time:".len()..].trim().to_string());
            } else if line.starts_with("# User@Host:") {
                entry.user_host = Some(line["# User@Host:".len()..].trim().to_string());
            } else if line.starts_with("# Query_time:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for i in 0..parts.len() {
                    if parts[i] == "Query_time:" && i + 1 < parts.len() {
                        entry.query_time = parts[i + 1].parse().unwrap_or(0.0);
                    } else if parts[i] == "Lock_time:" && i + 1 < parts.len() {
                        entry.lock_time = parts[i + 1].parse().unwrap_or(0.0);
                    } else if parts[i] == "Rows_sent:" && i + 1 < parts.len() {
                        entry.rows_sent = parts[i + 1].parse().unwrap_or(0);
                    } else if parts[i] == "Rows_examined:" && i + 1 < parts.len() {
                        entry.rows_examined = parts[i + 1].parse().unwrap_or(0);
                    }
                }
            } else if line.starts_with('#') {
                // Other comments, ignore
            } else {
                let trimmed = line.trim();
                // Filter out specific meta statements if they appear on their own line
                if trimmed.starts_with("SET timestamp=") && trimmed.ends_with(';') {
                    // skip
                } else if trimmed.starts_with("use ") && trimmed.ends_with(';') {
                    // skip
                } else if !trimmed.is_empty() {
                    sql_lines.push(line);
                }
            }
        }

        if has_data {
            entry.sql = sql_lines.join("\n").trim().to_string();
            // In case we only had headers/comments but no SQL, still return if has_data
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }
}

pub fn normalize_query(query: &str) -> String {
    let mut normalized = String::with_capacity(query.len());
    let mut chars = query.chars().peekable();
    let mut prev_char = ' ';

    while let Some(c) = chars.next() {
        if c == '\'' || c == '"' {
            if prev_char != '?' {
                normalized.push('?');
                prev_char = '?';
            }
            let quote = c;
            let mut escaped = false;
            while let Some(inner) = chars.next() {
                if escaped {
                    escaped = false;
                } else if inner == '\\' {
                    escaped = true;
                } else if inner == quote {
                    break;
                }
            }
        } else if c.is_ascii_digit() {
            if prev_char.is_ascii_alphabetic()
                || prev_char == '_'
                || prev_char.is_ascii_digit()
                || prev_char == '$'
            {
                let lc = c.to_ascii_lowercase();
                normalized.push(lc);
                prev_char = lc;
            } else {
                if prev_char != '?' {
                    normalized.push('?');
                    prev_char = '?';
                }
                while let Some(&next_c) = chars.peek() {
                    if next_c.is_ascii_digit() || next_c == '.' {
                        chars.next();
                    } else {
                        break;
                    }
                }
            }
        } else {
            if c.is_whitespace() {
                if prev_char != ' ' {
                    normalized.push(' ');
                    prev_char = ' ';
                }
            } else {
                let lc = c.to_ascii_lowercase();
                normalized.push(lc);
                prev_char = lc;
            }
        }
    }
    normalized.trim().to_string()
}

#[derive(Debug, Clone, Default)]
pub struct QueryStats {
    pub normalized_query: String,
    pub total_time: f64,
    pub execution_count: u64,
    pub total_lock_time: f64,
    pub total_rows_sent: u64,
    pub total_rows_examined: u64,
}

impl QueryStats {
    pub fn average_latency(&self) -> f64 {
        if self.execution_count == 0 {
            0.0
        } else {
            self.total_time / self.execution_count as f64
        }
    }
}

pub struct QueryRanker {
    pub stats: HashMap<String, QueryStats>,
}

impl Default for QueryRanker {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryRanker {
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
        }
    }

    pub fn add_entry(&mut self, entry: &SlowQueryLogEntry) {
        if entry.sql.is_empty() {
            return;
        }
        let normalized = normalize_query(&entry.sql);
        let stat = self
            .stats
            .entry(normalized.clone())
            .or_insert_with(|| QueryStats {
                normalized_query: normalized,
                total_time: 0.0,
                execution_count: 0,
                total_lock_time: 0.0,
                total_rows_sent: 0,
                total_rows_examined: 0,
            });

        stat.total_time += entry.query_time;
        stat.execution_count += 1;
        stat.total_lock_time += entry.lock_time;
        stat.total_rows_sent += entry.rows_sent;
        stat.total_rows_examined += entry.rows_examined;
    }

    pub fn top_queries_by_time(&self, limit: usize) -> Vec<QueryStats> {
        let mut queries: Vec<_> = self.stats.values().cloned().collect();
        queries.sort_by(|a, b| {
            b.total_time
                .partial_cmp(&a.total_time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        queries.truncate(limit);
        queries
    }
}
