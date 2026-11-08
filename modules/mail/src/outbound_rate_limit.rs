#![forbid(unsafe_code)]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionRequest {
    pub sender: String,
    pub recipient_count: usize,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitDecision {
    Allowed {
        remaining: u32,
        reset_after_secs: u64,
    },
    Exceeded {
        reset_after_secs: u64,
        limit: u32,
    },
}

#[derive(Clone, Debug)]
pub struct QuotaConfig {
    pub limit: u32,
    pub window_secs: u64,
}

struct Record {
    timestamp: u64,
    count: usize,
}

pub struct OutboundRateLimiter {
    mailbox_limits: HashMap<String, QuotaConfig>,
    domain_limits: HashMap<String, QuotaConfig>,
    state: Arc<Mutex<RateLimitState>>,
}

struct RateLimitState {
    mailbox_records: HashMap<String, VecDeque<Record>>,
    domain_records: HashMap<String, VecDeque<Record>>,
}

impl OutboundRateLimiter {
    pub fn new(
        mailbox_limits: HashMap<String, QuotaConfig>,
        domain_limits: HashMap<String, QuotaConfig>,
    ) -> Self {
        Self {
            mailbox_limits,
            domain_limits,
            state: Arc::new(Mutex::new(RateLimitState {
                mailbox_records: HashMap::new(),
                domain_records: HashMap::new(),
            })),
        }
    }

    fn domain_from_sender(sender: &str) -> String {
        sender.split('@').last().unwrap_or(sender).to_string()
    }

    pub fn evaluate(&self, req: &SubmissionRequest) -> RateLimitDecision {
        let domain = Self::domain_from_sender(&req.sender);

        let mut state = self.state.lock().unwrap();

        // 1. Cleanup expired
        if let Some(quota) = self.mailbox_limits.get(&req.sender) {
            let records = state.mailbox_records.entry(req.sender.clone()).or_default();
            Self::cleanup_records(records, req.timestamp, quota.window_secs);
        }
        if let Some(quota) = self.domain_limits.get(&domain) {
            let records = state.domain_records.entry(domain.clone()).or_default();
            Self::cleanup_records(records, req.timestamp, quota.window_secs);
        }

        // 2. Compute current usage
        let mb_usage = state.mailbox_records.get(&req.sender).map(|r| r.iter().map(|x| x.count).sum::<usize>()).unwrap_or(0);
        let dom_usage = state.domain_records.get(&domain).map(|r| r.iter().map(|x| x.count).sum::<usize>()).unwrap_or(0);

        // 3. Check for exceed
        let mut exceed_decision = None;

        let check_limit = |usage: usize, req_count: usize, quota: &QuotaConfig, records: Option<&VecDeque<Record>>| -> Option<RateLimitDecision> {
            if usage + req_count > quota.limit as usize {
                let reset = Self::calculate_reset(records.unwrap(), req.timestamp, quota.window_secs, quota.limit as usize, usage, req_count);
                Some(RateLimitDecision::Exceeded {
                    reset_after_secs: reset,
                    limit: quota.limit,
                })
            } else {
                None
            }
        };

        if let Some(quota) = self.mailbox_limits.get(&req.sender) {
            if let Some(exc) = check_limit(mb_usage, req.recipient_count, quota, state.mailbox_records.get(&req.sender)) {
                exceed_decision = Some(exc);
            }
        }

        if exceed_decision.is_none() {
            if let Some(quota) = self.domain_limits.get(&domain) {
                if let Some(exc) = check_limit(dom_usage, req.recipient_count, quota, state.domain_records.get(&domain)) {
                    exceed_decision = Some(exc);
                }
            }
        }

        if let Some(exc) = exceed_decision {
            return exc;
        }

        // 4. Record the request
        if self.mailbox_limits.contains_key(&req.sender) {
            state.mailbox_records.get_mut(&req.sender).unwrap().push_back(Record {
                timestamp: req.timestamp,
                count: req.recipient_count,
            });
        }
        if self.domain_limits.contains_key(&domain) {
            state.domain_records.get_mut(&domain).unwrap().push_back(Record {
                timestamp: req.timestamp,
                count: req.recipient_count,
            });
        }

        // 5. Calculate allow metrics
        let mut min_remaining = u32::MAX;
        let mut max_reset = 0;

        if let Some(quota) = self.mailbox_limits.get(&req.sender) {
            let rem = (quota.limit as usize - (mb_usage + req.recipient_count)) as u32;
            if rem < min_remaining {
                min_remaining = rem;
            }
            let reset = Self::calculate_reset_for_allowed(state.mailbox_records.get(&req.sender).unwrap(), req.timestamp, quota.window_secs);
            if reset > max_reset {
                max_reset = reset;
            }
        }

        if let Some(quota) = self.domain_limits.get(&domain) {
            let rem = (quota.limit as usize - (dom_usage + req.recipient_count)) as u32;
            if rem < min_remaining {
                min_remaining = rem;
            }
            let reset = Self::calculate_reset_for_allowed(state.domain_records.get(&domain).unwrap(), req.timestamp, quota.window_secs);
            if reset > max_reset {
                max_reset = reset;
            }
        }

        RateLimitDecision::Allowed {
            remaining: if min_remaining == u32::MAX { u32::MAX } else { min_remaining },
            reset_after_secs: max_reset,
        }
    }

    fn cleanup_records(records: &mut VecDeque<Record>, now: u64, window: u64) {
        let cutoff = now.saturating_sub(window);
        while let Some(front) = records.front() {
            if front.timestamp <= cutoff {
                records.pop_front();
            } else {
                break;
            }
        }
    }

    fn calculate_reset(
        records: &VecDeque<Record>,
        now: u64,
        window: u64,
        limit: usize,
        current_usage: usize,
        requested: usize,
    ) -> u64 {
        let mut needed = (current_usage + requested).saturating_sub(limit);
        if needed == 0 {
            return 0;
        }

        for r in records {
            if r.count >= needed {
                let expiration_time = r.timestamp + window;
                return expiration_time.saturating_sub(now);
            } else {
                needed -= r.count;
            }
        }

        window
    }

    fn calculate_reset_for_allowed(
        records: &VecDeque<Record>,
        now: u64,
        window: u64,
    ) -> u64 {
        if let Some(front) = records.front() {
            let expiration_time = front.timestamp + window;
            expiration_time.saturating_sub(now)
        } else {
            0
        }
    }

    pub fn cleanup_all_expired(&self, now: u64) {
        let mut state = self.state.lock().unwrap();

        let mut empty_mailboxes = Vec::new();
        for (mailbox, records) in state.mailbox_records.iter_mut() {
            if let Some(quota) = self.mailbox_limits.get(mailbox) {
                Self::cleanup_records(records, now, quota.window_secs);
            }
            if records.is_empty() {
                empty_mailboxes.push(mailbox.clone());
            }
        }
        for mailbox in empty_mailboxes {
            state.mailbox_records.remove(&mailbox);
        }

        let mut empty_domains = Vec::new();
        for (domain, records) in state.domain_records.iter_mut() {
            if let Some(quota) = self.domain_limits.get(domain) {
                Self::cleanup_records(records, now, quota.window_secs);
            }
            if records.is_empty() {
                empty_domains.push(domain.clone());
            }
        }
        for domain in empty_domains {
            state.domain_records.remove(&domain);
        }
    }
}
