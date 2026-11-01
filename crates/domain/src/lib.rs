#![forbid(unsafe_code)]

//! Core domain contract for FusionPanel.

pub mod domain_name;
pub mod error;
pub mod id;
pub mod site;
pub mod state;

pub use domain_name::DomainName;
pub use error::DomainError;
pub use id::{JobId, NodeId, SiteId, TenantId};
pub use site::{ResourceOwner, SiteKind};
pub use state::ResourceState;
