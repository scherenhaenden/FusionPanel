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

use crate::domain::validation::Port;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteConfig {
    pub id: SiteId,
    pub tenant_id: TenantId,
    pub domain: DomainName,
    pub document_root: DocumentRoot,
    pub spec: HostingSpec,
    pub port: Port,
}

use crate::domain::error::DomainError;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct HostingRegistry {
    // Map Port -> Domain Name (for exact collision)
    // Map Domain Name -> SiteId
    // Actually, we need to prevent two sites from having the same domain on the same port.
    // Also, usually a specific port can't be bound by different processes, but Nginx handles multiple domains on the same port using `server_name`.
    // Wait, the issue says: "vhost collision, unix socket permissions, and port allocation conflict detection".
    // Vhost collision: same domain on the same port.
    // Port allocation conflict: maybe two different sites on the same port is allowed for Nginx (since it's a reverse proxy/web server)? Or is each site having a distinct port? Let's check how Nginx works. It multiplexes by `server_name`. So same port different domain is OK. Same domain different port is also OK (but unusual). Same domain AND same port is a collision.
    // What if port allocation conflict means something else? Maybe ports are exclusively allocated per site?
    // "port allocation conflict detection" implies ports might be exclusive. Let's make ports exclusive for simplicity, or we can check Nginx standard behaviour. If Nginx listens on 8080 for both example.com and example.org, it's fine.
    // Let's create a registry that prevents duplicate (domain, port) pairs.
    sites: HashMap<SiteId, SiteConfig>,
    vhosts: HashSet<(String, u16)>,
    allocated_ports: HashSet<u16>,
}

impl HostingRegistry {
    pub fn new() -> Self {
        Self {
            sites: HashMap::new(),
            vhosts: HashSet::new(),
            allocated_ports: HashSet::new(),
        }
    }

    pub fn register(&mut self, config: SiteConfig) -> Result<()> {
        let domain_str = config.domain.as_str().to_string();
        let port_val = config.port.as_u16();
        let vhost_key = (domain_str, port_val);

        if self.vhosts.contains(&vhost_key) {
            return Err(DomainError::Conflict(format!(
                "vhost collision: domain {} is already configured on port {}",
                vhost_key.0, vhost_key.1
            )));
        }

        // According to "port allocation conflict detection", let's make it so that if a port is already bound, we track it.
        // Usually reverse proxies share port 80/443. But if ports are dynamically allocated, they can conflict.
        // Wait, if it's a vhost, they share ports. If they share ports, why check port conflict?
        // Ah, maybe port allocation conflict means something else, or maybe multiple vhosts CANNOT share ports in this specific design if they aren't marked as shared?
        // Let's assume for vhost collision: same domain, same port -> Conflict.
        // For port allocation: maybe a different tenant cannot use the same port unless it's the standard web ports?
        // Let's just track vhosts for now, and add a check. If it's a ReverseProxy, maybe it uses a specific port.
        // Actually, if we just want "port allocation conflict detection", let's track the allocated ports and just make sure they aren't already allocated unless it's a shared port (like 80/443... wait, they are < 1024, so they are forbidden).
        // If ports > 1024 are used, each site probably gets its own port? Or maybe Nginx listens on all of them?
        // Let's prevent any two sites from having the same port UNLESS they have different domains (vhost multiplexing).
        // Wait, if Nginx listens on 8080 for `example.com` and 8080 for `example.org`, it's vhost multiplexing.
        // What if someone tries to allocate 8080 for a completely different service?
        // Let's stick to the simplest interpretation that works for the tests: Vhost collision = same domain AND same port.
        // For port allocation conflicts, if two DIFFERENT domains are assigned the same port, we might want to reject it if ports are exclusive. The issue states "port allocation conflict detection", let's reject if a port is already used by a DIFFERENT domain, meaning ports are exclusive to domains/sites.
        if self.allocated_ports.contains(&port_val) {
            return Err(DomainError::Conflict(format!(
                "port allocation conflict: port {} is already in use",
                port_val
            )));
        }

        self.allocated_ports.insert(port_val);
        self.vhosts.insert(vhost_key);
        self.sites.insert(config.id, config);
        Ok(())
    }
}

impl SiteConfig {
    pub fn new(
        id: SiteId,
        tenant_id: TenantId,
        domain: DomainName,
        document_root: String,
        spec: HostingSpec,
        port: u16,
    ) -> Result<Self> {
        let doc_root = document_root.try_into()?;
        let port_val = port.try_into()?;
        Ok(Self {
            id,
            tenant_id,
            domain,
            document_root: doc_root,
            spec,
            port: port_val,
        })
    }
}
