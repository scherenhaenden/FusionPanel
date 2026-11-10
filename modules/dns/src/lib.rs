#![forbid(unsafe_code)]

pub mod acme_dns;
pub mod axfr_acl;
pub mod backend;
pub mod caa_record;
pub mod dns_over_tls;
pub mod cloudflare_sync;
pub mod dnssec_mgr;
pub mod dnssec_key_rollover;
pub mod domain;
pub mod forwarders;
pub mod provider;
pub mod split_horizon;
pub mod idn_punycode;
pub mod powerdns_sync;
pub mod rrl_config;
pub mod route53_sync;
pub mod rpz_compiler;
pub mod notify_engine;
pub use notify_engine::*;
pub use route53_sync::*;
pub mod ttl_timer;
pub mod validation;
pub mod zone_file;
pub mod tsig_keyring;
pub use tsig_keyring::*;

pub use cloudflare_sync::*;
pub use rpz_compiler::*;
pub use powerdns_sync::*;

pub use rrl_config::*;

#[cfg(test)]
mod tests;
