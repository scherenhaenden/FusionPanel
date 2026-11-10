#![forbid(unsafe_code)]

pub mod clamav_scanner;
pub use clamav_scanner::*;

pub mod aliases;
pub mod autodiscover;
pub mod config;
pub mod connection_guard;
pub mod delivery;
pub mod dkim;
pub mod dkim_rotation;
pub mod dmarc;
pub mod dns_audit;
pub mod dns_records;
pub mod domain;
pub mod error;
pub mod models;
pub mod outbound_rate_limit;
pub mod plan;
pub mod quota;
pub mod quota_dict;
pub mod sieve;
pub mod sieve_validator;
pub mod spf_flatten;

#[cfg(test)]
mod tests;
