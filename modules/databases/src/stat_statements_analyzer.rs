#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq)]
pub struct PgStatRecord {
    pub queryid: i64,
    pub total_exec_time: f64,
    pub mean_exec_time: f64,
    pub calls: i64,
    pub shared_blks_hit: i64,
    pub shared_blks_read: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegressionAlert {
    pub queryid: i64,
    pub baseline_mean: f64,
    pub current_mean: f64,
    pub degradation_factor: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryJitter {
    pub queryid: i64,
    pub jitter: f64,
}

pub fn analyze_pg_stat_statements(
    baseline: &[PgStatRecord],
    current: &[PgStatRecord],
) -> Vec<RegressionAlert> {
    let mut alerts = Vec::new();

    for b in baseline {
        if let Some(c) = current.iter().find(|x| x.queryid == b.queryid) {
            if b.mean_exec_time > 0.0 {
                let degradation = c.mean_exec_time / b.mean_exec_time;
                if degradation > 3.0 {
                    alerts.push(RegressionAlert {
                        queryid: b.queryid,
                        baseline_mean: b.mean_exec_time,
                        current_mean: c.mean_exec_time,
                        degradation_factor: degradation,
                    });
                }
            }
        }
    }

    alerts
}

pub fn calculate_execution_jitter(records: &[PgStatRecord]) -> Vec<QueryJitter> {
    let mut jitters = Vec::new();
    for r in records {
        if r.calls > 0 {
            let jitter = (r.total_exec_time / (r.calls as f64) - r.mean_exec_time).abs();
            jitters.push(QueryJitter {
                queryid: r.queryid,
                jitter,
            });
        }
    }
    jitters
}

pub fn parse_pg_stat_statements(csv_data: &str) -> Result<Vec<PgStatRecord>, String> {
    let mut records = Vec::new();
    let mut is_header = true;
    for line in csv_data.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if is_header && line.contains("queryid") {
            is_header = false;
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 6 {
            continue;
        }

        let queryid = parts[0].trim().parse::<i64>().map_err(|e| e.to_string())?;
        let total_exec_time = parts[1].trim().parse::<f64>().map_err(|e| e.to_string())?;
        let mean_exec_time = parts[2].trim().parse::<f64>().map_err(|e| e.to_string())?;
        let calls = parts[3].trim().parse::<i64>().map_err(|e| e.to_string())?;
        let shared_blks_hit = parts[4].trim().parse::<i64>().map_err(|e| e.to_string())?;
        let shared_blks_read = parts[5].trim().parse::<i64>().map_err(|e| e.to_string())?;

        records.push(PgStatRecord {
            queryid,
            total_exec_time,
            mean_exec_time,
            calls,
            shared_blks_hit,
            shared_blks_read,
        });
    }

    Ok(records)
}
