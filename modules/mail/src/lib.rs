#![forbid(unsafe_code)]

pub mod aliases;
pub mod config;
pub mod delivery;
pub mod dkim;
pub mod dkim_rotation;
pub mod dmarc;
pub mod dns_records;
pub mod domain;
pub mod error;
pub mod models;
pub mod plan;
pub mod quota;

#[cfg(test)]
mod tests;
