use serde::{Deserialize, Serialize};
use crate::domain::{TenantId, SiteId, DomainName, validate_document_root, error::Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteKind {
    Php,
    Static,
    ReverseProxy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhpConfig {
    pub version: String,
    pub pool_name: String,
    pub memory_limit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReverseProxyConfig {
    pub upstream_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HostingSpec {
    Php(PhpConfig),
    Static(StaticConfig),
    ReverseProxy(ReverseProxyConfig),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteConfig {
    pub id: SiteId,
    pub tenant_id: TenantId,
    pub domain: DomainName,
    pub document_root: String,
    pub spec: HostingSpec,
}

impl SiteConfig {
    pub fn new(id: SiteId, tenant_id: TenantId, domain: DomainName, document_root: String, spec: HostingSpec) -> Result<Self> {
        validate_document_root(&document_root)?;
        Ok(Self {
            id,
            tenant_id,
            domain,
            document_root,
            spec,
        })
    }
}
