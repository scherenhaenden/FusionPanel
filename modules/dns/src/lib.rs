#![forbid(unsafe_code)]

pub mod domain;
pub mod dnssec_mgr;
pub mod provider;
pub mod backend;
pub mod zone_file;
pub mod validation;
pub mod acme_dns;

#[cfg(test)]
mod tests;
