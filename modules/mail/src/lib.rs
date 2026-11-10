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
pub mod fts_indexer;
pub mod milter_rate_limit;
pub mod maildir_flags;
pub mod milter_headers;
pub mod lmtp_routing;
pub mod models;

pub use lmtp_routing::*;
pub mod outbound_rate_limit;
pub mod plan;
pub mod quota;
pub mod quota_dict;
pub mod relay_transport;
pub mod rspamd_rules;
pub mod sasl_submission;
pub use sasl_submission::*;
pub mod sieve;
pub mod sieve_vacation;
pub mod sieve_validator;
pub mod spf_flatten;

pub use fts_indexer::*;
pub use maildir_flags::*;
pub use milter_headers::*;
pub use relay_transport::*;
pub use rspamd_rules::*;

#[cfg(test)]
mod tests;
