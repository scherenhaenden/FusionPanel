#![forbid(unsafe_code)]

pub mod acme_dns;
pub mod axfr_acl;
pub mod backend;
pub mod caa_record;
pub mod dnssec_mgr;
pub mod domain;
pub mod forwarders;
pub mod provider;
pub mod split_horizon;
pub mod ttl_timer;
pub mod validation;
pub mod zone_file;
pub mod tsig_keyring;
pub use tsig_keyring::*;

#[cfg(test)]
mod tests;
