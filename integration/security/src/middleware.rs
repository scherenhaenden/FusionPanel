#![forbid(unsafe_code)]

use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::{header, Response, StatusCode},
    middleware::Next,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct RateLimiter {
    // Token Bucket: (tokens, last_refill)
    ip_buckets: Arc<Mutex<HashMap<String, (f64, Instant)>>>,
    token_buckets: Arc<Mutex<HashMap<String, (f64, Instant)>>>,
    capacity: f64,
    refill_rate: f64, // tokens per second
}

impl RateLimiter {
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            ip_buckets: Arc::new(Mutex::new(HashMap::new())),
            token_buckets: Arc::new(Mutex::new(HashMap::new())),
            capacity,
            refill_rate,
        }
    }

    async fn check_limit(
        &self,
        key: &str,
        buckets: &Arc<Mutex<HashMap<String, (f64, Instant)>>>,
    ) -> bool {
        let mut buckets = buckets.lock().await;
        let now = Instant::now();
        let (tokens, last_refill) = buckets
            .entry(key.to_string())
            .or_insert((self.capacity, now));

        let elapsed = now.duration_since(*last_refill).as_secs_f64();
        let mut new_tokens = *tokens + elapsed * self.refill_rate;
        if new_tokens > self.capacity {
            new_tokens = self.capacity;
        }

        if new_tokens >= 1.0 {
            *tokens = new_tokens - 1.0;
            *last_refill = now;
            true
        } else {
            *tokens = new_tokens;
            *last_refill = now;
            false
        }
    }

    pub async fn check_ip(&self, ip: &str) -> bool {
        self.check_limit(ip, &self.ip_buckets).await
    }

    pub async fn check_token(&self, token: &str) -> bool {
        self.check_limit(token, &self.token_buckets).await
    }
}

#[derive(Clone)]
pub struct SecurityState {
    pub rate_limiter: RateLimiter,
    pub audit_log: crate::audit::AuditLog,
    pub firewall: crate::firewall::Firewall,
}

pub async fn rate_limit_middleware(
    State(state): State<Arc<SecurityState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Result<Response<Body>, StatusCode> {
    let ip = addr.ip().to_string();

    // Check firewall first
    if state.firewall.check_ip(addr.ip()).await == crate::firewall::RuleAction::Deny {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut allowed = state.rate_limiter.check_ip(&ip).await;

    // Check token rate limiting if Authorization header is present
    if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if !state.rate_limiter.check_token(token).await {
                    allowed = false;
                }
            }
        }
    }

    if allowed {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::TOO_MANY_REQUESTS)
    }
}
