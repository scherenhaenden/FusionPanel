#![forbid(unsafe_code)]

pub mod aliases;
pub mod config;
pub mod delivery;
pub mod dns_records;
pub mod domain;
pub mod error;
pub mod models;
pub mod plan;

#[cfg(test)]
mod tests;
