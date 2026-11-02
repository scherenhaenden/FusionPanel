use crate::domain::validation::{DocumentRoot, MemoryLimit, PhpVersion, PoolName, UpstreamUrl};
use crate::domain::{error::Result, DomainName, SiteId, TenantId};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteKind {
    Php,
    Static,
    ReverseProxy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhpConfig {
    pub version: PhpVersion,
    pub pool_name: PoolName,
    pub memory_limit: MemoryLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReverseProxyConfig {
    pub upstream_url: UpstreamUrl,
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
    pub document_root: DocumentRoot,
    pub spec: HostingSpec,
}

impl SiteConfig {
    pub fn new(
        id: SiteId,
        tenant_id: TenantId,
        domain: DomainName,
        document_root: String,
        spec: HostingSpec,
    ) -> Result<Self> {
        let doc_root = document_root.try_into()?;
        Ok(Self {
            id,
            tenant_id,
            domain,
            document_root: doc_root,
            spec,
        })
    }
}
