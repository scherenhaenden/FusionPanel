#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq)]
pub struct SlowQueryLogEntry {
    pub query_time: f64,
    pub lock_time: f64,
    pub rows_sent: u64,
    pub rows_examined: u64,
    pub query: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnomalyScore {
    pub examined_to_sent_ratio: f64,
    pub score: f64,
    pub index_recommended: bool,
}

pub fn parse_slow_query_logs(log: &str) -> Vec<SlowQueryLogEntry> {
    let mut entries = Vec::new();
    let mut current_entry: Option<SlowQueryLogEntry> = None;
    let mut current_query = String::new();

    for line in log.lines() {
        if line.starts_with("# Time:") || line.starts_with("# User@Host:") || line.starts_with("# Thread_id:") {
            continue;
        }

        if line.starts_with("# Query_time:") {
            if let Some(mut entry) = current_entry.take() {
                entry.query = current_query.trim().to_string();
                if !entry.query.is_empty() || entry.query_time > 0.0 {
                    entries.push(entry);
                }
                current_query.clear();
            }

            let mut query_time = 0.0;
            let mut lock_time = 0.0;
            let mut rows_sent = 0;
            let mut rows_examined = 0;

            let parts: Vec<&str> = line.split_whitespace().collect();
            for i in 0..parts.len() {
                match parts[i] {
                    "Query_time:" => {
                        if i + 1 < parts.len() {
                            query_time = parts[i + 1].parse().unwrap_or(0.0);
                        }
                    }
                    "Lock_time:" => {
                        if i + 1 < parts.len() {
                            lock_time = parts[i + 1].parse().unwrap_or(0.0);
                        }
                    }
                    "Rows_sent:" => {
                        if i + 1 < parts.len() {
                            rows_sent = parts[i + 1].parse().unwrap_or(0);
                        }
                    }
                    "Rows_examined:" => {
                        if i + 1 < parts.len() {
                            rows_examined = parts[i + 1].parse().unwrap_or(0);
                        }
                    }
                    _ => {}
                }
            }

            current_entry = Some(SlowQueryLogEntry {
                query_time,
                lock_time,
                rows_sent,
                rows_examined,
                query: String::new(),
            });
            continue;
        }

        if line.starts_with("SET timestamp=") || line.starts_with("use ") {
            continue;
        }

        if current_entry.is_some() && !line.starts_with('#') {
            if !current_query.is_empty() {
                current_query.push('\n');
            }
            current_query.push_str(line);
        }
    }

    if let Some(mut entry) = current_entry.take() {
        entry.query = current_query.trim().to_string();
        if !entry.query.is_empty() || entry.query_time > 0.0 {
            entries.push(entry);
        }
    }

    entries
}

pub fn analyze_slow_query(entry: &SlowQueryLogEntry) -> AnomalyScore {
    let sent = if entry.rows_sent == 0 { 1.0 } else { entry.rows_sent as f64 };
    let examined = entry.rows_examined as f64;
    let examined_to_sent_ratio = examined / sent;

    let mut score = (examined_to_sent_ratio * 0.1) + (entry.query_time * 10.0);
    if entry.rows_examined == 0 {
        score = 0.0;
    }

    let index_recommended = examined_to_sent_ratio > 100.0 && entry.rows_examined > 1000 && entry.query_time > 0.5;

    AnomalyScore {
        examined_to_sent_ratio,
        score,
        index_recommended,
    }
}
