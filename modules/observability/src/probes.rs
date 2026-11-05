use reqwest::blocking::Client;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

fn bounded_attempt_timeout(
    total_timeout: Duration,
    elapsed: Duration,
    per_attempt_budget: Duration,
) -> Option<Duration> {
    let remaining = total_timeout.checked_sub(elapsed)?;
    if remaining.is_zero() {
        None
    } else {
        Some(std::cmp::min(per_attempt_budget, remaining))
    }
}

pub struct ProbeResult {
    pub success: bool,
    pub latency_ms: u64,
    pub status_code: Option<u16>,
    pub error: Option<String>,
}

pub fn check_http(url: &str, timeout_secs: u64) -> ProbeResult {
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .unwrap_or_else(|_| Client::new());

    let start = Instant::now();
    match client.get(url).send() {
        Ok(response) => {
            let latency = start.elapsed().as_millis() as u64;
            let status = response.status();
            ProbeResult {
                success: status.is_success(),
                latency_ms: latency,
                status_code: Some(status.as_u16()),
                error: None,
            }
        }
        Err(e) => ProbeResult {
            success: false,
            latency_ms: start.elapsed().as_millis() as u64,
            status_code: None,
            error: Some(e.to_string()),
        },
    }
}

pub fn check_tcp(address: &str, timeout_secs: u64) -> ProbeResult {
    let start = Instant::now();
    let total_timeout = Duration::from_secs(timeout_secs);

    // Resolve hostnames
    let addrs: Vec<_> = match address.to_socket_addrs() {
        Ok(addrs) => addrs.take(5).collect(),
        Err(e) => {
            return ProbeResult {
                success: false,
                latency_ms: start.elapsed().as_millis() as u64,
                status_code: None,
                error: Some(format!("DNS resolution failed: {}", e)),
            }
        }
    };

    if addrs.is_empty() {
        return ProbeResult {
            success: false,
            latency_ms: start.elapsed().as_millis() as u64,
            status_code: None,
            error: Some("No addresses found".to_string()),
        };
    }

    let mut last_error = None;
    let per_attempt_budget = {
        let budget = total_timeout / (addrs.len() as u32);
        if budget.is_zero() {
            Duration::from_millis(1)
        } else {
            budget
        }
    };

    for addr in &addrs {
        let elapsed = start.elapsed();
        if elapsed >= total_timeout {
            return ProbeResult {
                success: false,
                latency_ms: elapsed.as_millis() as u64,
                status_code: None,
                error: Some("Timeout exceeded before trying all addresses".to_string()),
            };
        }

        // DNS resolution and earlier attempts consume the same total probe
        // deadline. Never give a later address a fresh full per-address budget.
        let Some(attempt_timeout) =
            bounded_attempt_timeout(total_timeout, elapsed, per_attempt_budget)
        else {
            return ProbeResult {
                success: false,
                latency_ms: elapsed.as_millis() as u64,
                status_code: None,
                error: Some("Timeout exceeded before trying all addresses".to_string()),
            };
        };
        match TcpStream::connect_timeout(addr, attempt_timeout) {
            Ok(_) => {
                return ProbeResult {
                    success: true,
                    latency_ms: start.elapsed().as_millis() as u64,
                    status_code: None,
                    error: None,
                }
            }
            Err(e) => last_error = Some(e),
        }
    }

    ProbeResult {
        success: false,
        latency_ms: start.elapsed().as_millis() as u64,
        status_code: None,
        error: last_error.map(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::bounded_attempt_timeout;
    use std::time::Duration;

    #[test]
    fn tcp_candidate_budget_is_capped_by_remaining_probe_deadline() {
        assert_eq!(
            bounded_attempt_timeout(
                Duration::from_secs(1),
                Duration::from_millis(950),
                Duration::from_millis(200),
            ),
            Some(Duration::from_millis(50))
        );
        assert_eq!(
            bounded_attempt_timeout(
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_millis(200),
            ),
            None
        );
    }
}
