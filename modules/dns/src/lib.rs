#![forbid(unsafe_code)]

pub mod acme_dns;
pub mod axfr_acl;
pub mod backend;
pub mod caa_record;
pub mod dnssec_mgr;
pub mod domain;
pub mod forwarders;
pub mod powerdns_sync;
pub mod provider;
pub mod split_horizon;
pub mod ttl_timer;
pub mod validation;
pub mod zone_file;

pub use powerdns_sync::*;

#[cfg(test)]
mod tests;
