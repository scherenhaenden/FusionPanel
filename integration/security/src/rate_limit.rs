#![forbid(unsafe_code)]

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::time::{Duration, Instant};
use ipnet::IpNet;

pub struct SlidingWindow {
    window_size: Duration,
    max_requests: usize,
    requests: HashMap<String, VecDeque<Instant>>,
}

impl SlidingWindow {
    pub fn new(window_size: Duration, max_requests: usize) -> Self {
        Self {
            window_size,
            max_requests,
            requests: HashMap::new(),
        }
    }

    pub fn check_and_add(&mut self, key: &str, now: Instant) -> bool {
        let entry = self.requests.entry(key.to_string()).or_insert_with(VecDeque::new);

        // Remove old entries
        while let Some(&oldest) = entry.front() {
            if now.saturating_duration_since(oldest) > self.window_size {
                entry.pop_front();
            } else {
                break;
            }
        }

        if entry.len() >= self.max_requests {
            false
        } else {
            entry.push_back(now);
            true
        }
    }
}

pub struct BruteForceGuard {
    threshold: usize,
    base_delay: Duration,
    failures: HashMap<IpAddr, (usize, Instant)>,
}

impl BruteForceGuard {
    pub fn new(threshold: usize, base_delay: Duration) -> Self {
        Self {
            threshold,
            base_delay,
            failures: HashMap::new(),
        }
    }

    pub fn record_failure(&mut self, ip: IpAddr, now: Instant) {
        let entry = self.failures.entry(ip).or_insert((0, now));
        entry.0 += 1;
        entry.1 = now;
    }

    pub fn record_success(&mut self, ip: IpAddr) {
        self.failures.remove(&ip);
    }

    pub fn is_blocked(&self, ip: IpAddr, now: Instant) -> bool {
        if let Some(&(count, last_failure)) = self.failures.get(&ip) {
            if count > self.threshold {
                let multiplier = (count - self.threshold) as u32;
                let delay = self.base_delay.checked_mul(multiplier).unwrap_or(self.base_delay);
                if now.saturating_duration_since(last_failure) < delay {
                    return true;
                }
            }
        }
        false
    }
}

pub struct RateLimiter {
    whitelist: Vec<IpNet>,
    ip_limiter: SlidingWindow,
    key_limiter: SlidingWindow,
    brute_force_guard: BruteForceGuard,
}

impl RateLimiter {
    pub fn new(
        whitelist: Vec<IpNet>,
        ip_window: Duration,
        ip_max: usize,
        key_window: Duration,
        key_max: usize,
        brute_force_threshold: usize,
        brute_force_base_delay: Duration,
    ) -> Self {
        Self {
            whitelist,
            ip_limiter: SlidingWindow::new(ip_window, ip_max),
            key_limiter: SlidingWindow::new(key_window, key_max),
            brute_force_guard: BruteForceGuard::new(brute_force_threshold, brute_force_base_delay),
        }
    }

    fn is_whitelisted(&self, ip: IpAddr) -> bool {
        self.whitelist.iter().any(|net| net.contains(&ip))
    }

    pub fn check_ip(&mut self, ip: IpAddr, now: Instant) -> bool {
        if self.is_whitelisted(ip) {
            return true;
        }
        self.ip_limiter.check_and_add(&ip.to_string(), now)
    }

    pub fn check_key(&mut self, key: &str, now: Instant) -> bool {
        self.key_limiter.check_and_add(key, now)
    }

    pub fn check_login_allowed(&self, ip: IpAddr, now: Instant) -> bool {
        if self.is_whitelisted(ip) {
            return true;
        }
        !self.brute_force_guard.is_blocked(ip, now)
    }

    pub fn record_login_failure(&mut self, ip: IpAddr, now: Instant) {
        if !self.is_whitelisted(ip) {
            self.brute_force_guard.record_failure(ip, now);
        }
    }

    pub fn record_login_success(&mut self, ip: IpAddr) {
        self.brute_force_guard.record_success(ip);
    }
}
