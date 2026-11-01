use reqwest::blocking::Client;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

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

    let start = std::time::Instant::now();
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
    let start = std::time::Instant::now();

    // Resolve hostnames
    let addrs = match address.to_socket_addrs() {
        Ok(mut addrs) => addrs.next(),
        Err(e) => {
            return ProbeResult {
                success: false,
                latency_ms: start.elapsed().as_millis() as u64,
                status_code: None,
                error: Some(format!("DNS resolution failed: {}", e)),
            }
        }
    };

    let addr = match addrs {
        Some(a) => a,
        None => {
            return ProbeResult {
                success: false,
                latency_ms: start.elapsed().as_millis() as u64,
                status_code: None,
                error: Some("No addresses found".to_string()),
            }
        }
    };

    match TcpStream::connect_timeout(&addr, Duration::from_secs(timeout_secs)) {
        Ok(_) => ProbeResult {
            success: true,
            latency_ms: start.elapsed().as_millis() as u64,
            status_code: None,
            error: None,
        },
        Err(e) => ProbeResult {
            success: false,
            latency_ms: start.elapsed().as_millis() as u64,
            status_code: None,
            error: Some(e.to_string()),
        },
    }
}
