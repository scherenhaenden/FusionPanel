use reqwest::blocking::Client;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

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

    for (i, addr) in addrs.iter().enumerate() {
        let elapsed = start.elapsed();
        if elapsed >= total_timeout {
            return ProbeResult {
                success: false,
                latency_ms: elapsed.as_millis() as u64,
                status_code: None,
                error: Some("Timeout exceeded before trying all addresses".to_string()),
            };
        }

        let remaining = total_timeout - elapsed;
        let remaining_attempts = (addrs.len() - i) as u32;
        let attempt_budget = remaining / remaining_attempts;

        match TcpStream::connect_timeout(addr, attempt_budget) {
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
