#![forbid(unsafe_code)]

use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub struct InnodbMetrics {
    pub read_requests: u64,
    pub reads: u64,
    pub pages_total: u64,
}

#[derive(Debug, PartialEq, Clone)]
pub enum SizingAdvice {
    IncreaseBufferPool,
    DecreaseBufferPool,
    Optimal,
}

#[derive(Debug, PartialEq, Clone)]
pub struct AdvisorResult {
    pub hit_ratio: f64,
    pub advice: SizingAdvice,
}

pub fn parse_innodb_metrics(status_output: &str) -> Option<InnodbMetrics> {
    let mut metrics = HashMap::new();
    for line in status_output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let key = parts[0];
            let value_str = parts[1];
            if let Ok(val) = value_str.parse::<u64>() {
                metrics.insert(key.to_string(), val);
            }
        }
    }

    let read_requests = metrics.get("Innodb_buffer_pool_read_requests").cloned()?;
    let reads = metrics.get("Innodb_buffer_pool_reads").cloned()?;
    let pages_total = metrics.get("Innodb_buffer_pool_pages_total").cloned()?;

    Some(InnodbMetrics {
        read_requests,
        reads,
        pages_total,
    })
}

pub fn calculate_hit_ratio(metrics: &InnodbMetrics) -> f64 {
    if metrics.read_requests == 0 {
        return 100.0;
    }
    let reads = metrics.reads as f64;
    let requests = metrics.read_requests.max(1) as f64;
    100.0 - (reads * 100.0 / requests)
}

pub fn advise_buffer_pool_size(hit_ratio: f64, available_host_memory_bytes: u64, current_pool_size_bytes: u64) -> SizingAdvice {
    // Basic heuristic for advice based on miss trends and available memory
    if hit_ratio < 95.0 {
        // If hit ratio is low and we have memory, suggest increasing
        // E.g., if we can increase it without using up all available memory
        if available_host_memory_bytes > current_pool_size_bytes * 2 {
            return SizingAdvice::IncreaseBufferPool;
        }
    } else if hit_ratio > 99.9 {
        // If hit ratio is extremely high, we might be able to decrease it to free up memory
        // But only suggest if it's taking a significant chunk of memory
        if current_pool_size_bytes > available_host_memory_bytes / 2 {
             return SizingAdvice::DecreaseBufferPool;
        }
    }

    SizingAdvice::Optimal
}

pub fn evaluate_innodb_performance(status_output: &str, available_host_memory_bytes: u64, current_pool_size_bytes: u64) -> Option<AdvisorResult> {
    let metrics = parse_innodb_metrics(status_output)?;
    let hit_ratio = calculate_hit_ratio(&metrics);
    let advice = advise_buffer_pool_size(hit_ratio, available_host_memory_bytes, current_pool_size_bytes);

    Some(AdvisorResult {
        hit_ratio,
        advice,
    })
}
