#![forbid(unsafe_code)]

pub mod aliases;
pub mod config;
pub mod delivery;
pub mod dkim;
pub mod dkim_rotation;
pub mod dmarc;
pub mod dns_audit;
pub mod dns_records;
pub mod domain;
pub mod error;
pub mod models;
pub mod plan;
pub mod quota;
pub mod sieve;
pub mod spf_flatten;

#[cfg(test)]
mod tests;
