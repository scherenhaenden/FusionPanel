#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub request_id: Uuid,
    #[serde(flatten)]
    pub operation: Operation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", content = "payload", deny_unknown_fields)]
pub enum Operation {
    #[serde(rename = "system.info")]
    SystemInfo(SystemInfoPayload),
    #[serde(rename = "site.validate")]
    SiteValidate(SiteValidatePayload),
    #[serde(rename = "site.apply")]
    SiteApply(SiteApplyPayload),
    #[serde(rename = "site.delete")]
    SiteDelete(SiteDeletePayload),
    #[serde(rename = "tls.apply")]
    TlsApply(TlsApplyPayload),
    #[serde(rename = "tls.delete")]
    TlsDelete(TlsDeletePayload),
    #[serde(rename = "database.apply")]
    DatabaseApply(DatabaseApplyPayload),
    #[serde(rename = "database.delete")]
    DatabaseDelete(DatabaseDeletePayload),
    #[serde(rename = "database.user.create")]
    DatabaseUserCreate(DatabaseUserCreatePayload),
    #[serde(rename = "backup.create")]
    BackupCreate(BackupCreatePayload),
    #[serde(rename = "backup.restore")]
    BackupRestore(BackupRestorePayload),
    #[serde(rename = "dns.zone.apply")]
    DnsZoneApply(DnsZoneApplyPayload),
    #[serde(rename = "dns.zone.delete")]
    DnsZoneDelete(DnsZoneDeletePayload),
    #[serde(rename = "dns.probe")]
    DnsProbe(DnsProbePayload),
    #[serde(rename = "cron.apply")]
    CronApply(CronApplyPayload),
    #[serde(rename = "cron.delete")]
    CronDelete(CronDeletePayload),
    #[serde(rename = "cron.run_now")]
    CronRunNow(CronRunNowPayload),
    #[serde(rename = "container.apply")]
    ContainerApply(ContainerApplyPayload),
    #[serde(rename = "container.start")]
    ContainerStart(ContainerLifecyclePayload),
    #[serde(rename = "container.stop")]
    ContainerStop(ContainerLifecyclePayload),
    #[serde(rename = "container.restart")]
    ContainerRestart(ContainerLifecyclePayload),
    #[serde(rename = "container.delete")]
    ContainerDelete(ContainerDeletePayload),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SystemInfoPayload {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SiteValidatePayload {
    pub domain: String,
    pub document_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SiteApplyPayload {
    pub site_id: Uuid,
    pub domain: String,
    pub spec: SiteSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SiteSpec {
    #[serde(rename = "static")]
    Static { document_root: String },
    #[serde(rename = "reverse_proxy")]
    ReverseProxy { upstream_url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SiteDeletePayload {
    pub site_id: Uuid,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", deny_unknown_fields)]
pub enum TlsCertificateSource {
    #[serde(rename = "custom_pem")]
    CustomPem {
        certificate_pem: String,
        private_key_pem: String,
    },
    #[serde(rename = "acme_http01")]
    AcmeHttp01 {
        acme_directory_url: String,
        #[serde(default)]
        contact_email: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TlsApplyPayload {
    pub site_id: Uuid,
    pub domain: String,
    pub spec: SiteSpec,
    pub source: TlsCertificateSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TlsDeletePayload {
    pub site_id: Uuid,
    pub domain: String,
    pub spec: SiteSpec,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseEngineKind {
    Postgresql,
    Mariadb,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DatabaseApplyPayload {
    pub database_id: Uuid,
    pub name: String,
    pub engine: DatabaseEngineKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DatabaseDeletePayload {
    pub database_id: Uuid,
    pub name: String,
    pub engine: DatabaseEngineKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DatabaseUserCreatePayload {
    pub database_id: Uuid,
    pub database_name: String,
    pub engine: DatabaseEngineKind,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BackupTarget {
    Site {
        site_id: Uuid,
        domain: String,
        document_root: String,
    },
    Database {
        database_id: Uuid,
        name: String,
        engine: DatabaseEngineKind,
    },
    Bundle {
        site_id: Uuid,
        domain: String,
        document_root: String,
        database_id: Uuid,
        database_name: String,
        engine: DatabaseEngineKind,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BackupDestination {
    Local {
        base_dir: String,
    },
    S3 {
        endpoint: String,
        bucket: String,
        region: String,
        access_key: String,
        secret_key: String,
        #[serde(default)]
        prefix: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BackupCreatePayload {
    pub backup_id: Uuid,
    pub tenant_id: Uuid,
    pub archive_key: String,
    pub target: BackupTarget,
    pub destination: BackupDestination,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BackupRestorePayload {
    pub backup_id: Uuid,
    pub tenant_id: Uuid,
    pub archive_key: String,
    pub target: BackupTarget,
    pub destination: BackupDestination,
    #[serde(default)]
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum DnsRecordType {
    A,
    #[serde(rename = "AAAA")]
    Aaaa,
    #[serde(rename = "CNAME")]
    Cname,
    #[serde(rename = "MX")]
    Mx,
    #[serde(rename = "TXT")]
    Txt,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DnsRecordSpec {
    pub name: String,
    pub record_type: DnsRecordType,
    pub value: String,
    pub ttl: u32,
    #[serde(default)]
    pub priority: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DnsZoneApplyPayload {
    pub zone_id: Uuid,
    pub domain: String,
    pub serial: u32,
    pub default_ttl: u32,
    pub records: Vec<DnsRecordSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DnsZoneDeletePayload {
    pub zone_id: Uuid,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DnsProbePayload {
    pub zone_id: Uuid,
    pub domain: String,
    pub record_name: String,
    pub record_type: DnsRecordType,
    #[serde(default)]
    pub expected_substring: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CronJobSpec {
    pub cron_id: Uuid,
    pub name: String,
    pub schedule: String,
    pub command: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CronApplyPayload {
    pub tenant_id: Uuid,
    pub cron_id: Uuid,
    pub jobs: Vec<CronJobSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CronDeletePayload {
    pub tenant_id: Uuid,
    pub cron_id: Uuid,
    pub jobs: Vec<CronJobSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CronRunNowPayload {
    pub tenant_id: Uuid,
    pub cron_id: Uuid,
    pub command: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContainerPortProtocol {
    #[default]
    Tcp,
    Udp,
}

impl ContainerPortProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct ContainerPortMapping {
    pub host_port: u16,
    pub container_port: u16,
    #[serde(default)]
    pub protocol: ContainerPortProtocol,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContainerResourceLimits {
    pub cpu_millicores: u32,
    pub memory_bytes: u64,
    #[serde(default)]
    pub pids_limit: Option<u32>,
}

impl ContainerResourceLimits {
    pub const MIN_CPU_MILLICORES: u32 = 50;
    pub const MAX_CPU_MILLICORES: u32 = 64_000;
    pub const MIN_MEMORY_BYTES: u64 = 4 * 1024 * 1024; // 4 MiB
    pub const MAX_MEMORY_BYTES: u64 = 256 * 1024 * 1024 * 1024; // 256 GiB
    pub const MAX_PIDS_LIMIT: u32 = 32_768;

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.cpu_millicores < Self::MIN_CPU_MILLICORES
            || self.cpu_millicores > Self::MAX_CPU_MILLICORES
        {
            return Err(ValidationError::InvalidContainerResources);
        }
        if self.memory_bytes < Self::MIN_MEMORY_BYTES
            || self.memory_bytes > Self::MAX_MEMORY_BYTES
        {
            return Err(ValidationError::InvalidContainerResources);
        }
        if let Some(pids) = self.pids_limit {
            if pids == 0 || pids > Self::MAX_PIDS_LIMIT {
                return Err(ValidationError::InvalidContainerResources);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContainerRestartPolicy {
    No,
    Always,
    OnFailure,
    #[default]
    UnlessStopped,
}

impl ContainerRestartPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::No => "no",
            Self::Always => "always",
            Self::OnFailure => "on_failure",
            Self::UnlessStopped => "unless_stopped",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "no" => Some(Self::No),
            "always" => Some(Self::Always),
            "on_failure" => Some(Self::OnFailure),
            "unless_stopped" => Some(Self::UnlessStopped),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContainerApplyPayload {
    pub container_id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub image: String,
    #[serde(default)]
    pub ports: Vec<ContainerPortMapping>,
    pub resources: ContainerResourceLimits,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub restart_policy: ContainerRestartPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContainerLifecyclePayload {
    pub container_id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContainerDeletePayload {
    pub container_id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContainerOperationResult {
    pub container_id: Uuid,
    #[serde(default)]
    pub runtime_id: Option<String>,
    pub state: String,
    #[serde(default)]
    pub image_digest: Option<String>,
    #[serde(default)]
    pub ports: Vec<ContainerPortMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub version: u32,
    pub request_id: Uuid,
    #[serde(flatten)]
    pub result: ResponseResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResponseResult {
    Result(Value),
    Error(ResponseError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResponseError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ValidationError {
    UnsupportedVersion,
    InvalidDomain,
    InvalidDocumentRoot,
    InvalidUpstreamUrl,
    InvalidSiteId,
    InvalidCertificatePem,
    InvalidPrivateKeyPem,
    InvalidAcmeUrl,
    InvalidContactEmail,
    InvalidDatabaseId,
    InvalidDatabaseIdentifier,
    InvalidDatabasePassword,
    InvalidBackupId,
    InvalidBackupArchiveKey,
    InvalidBackupDestination,
    InvalidDnsZoneId,
    InvalidDnsRecord,
    InvalidDnsSerial,
    InvalidDnsTtl,
    InvalidCronJobId,
    InvalidCronSchedule,
    InvalidCronCommand,
    InvalidContainerId,
    InvalidTenantId,
    InvalidContainerName,
    InvalidContainerImage,
    InvalidContainerPorts,
    InvalidContainerResources,
    InvalidContainerEnv,
    InvalidContainerCommand,
}

impl Request {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.version != 1 {
            return Err(ValidationError::UnsupportedVersion);
        }
        match &self.operation {
            Operation::SystemInfo(_) => Ok(()),
            Operation::SiteValidate(payload) => payload.validate(),
            Operation::SiteApply(payload) => payload.validate(),
            Operation::SiteDelete(payload) => payload.validate(),
            Operation::TlsApply(payload) => payload.validate(),
            Operation::TlsDelete(payload) => payload.validate(),
            Operation::DatabaseApply(payload) => payload.validate(),
            Operation::DatabaseDelete(payload) => payload.validate(),
            Operation::DatabaseUserCreate(payload) => payload.validate(),
            Operation::BackupCreate(payload) => payload.validate(),
            Operation::BackupRestore(payload) => payload.validate(),
            Operation::DnsZoneApply(payload) => payload.validate(),
            Operation::DnsZoneDelete(payload) => payload.validate(),
            Operation::DnsProbe(payload) => payload.validate(),
            Operation::CronApply(payload) => payload.validate(),
            Operation::CronDelete(payload) => payload.validate(),
            Operation::CronRunNow(payload) => payload.validate(),
            Operation::ContainerApply(payload) => payload.validate(),
            Operation::ContainerStart(payload)
            | Operation::ContainerStop(payload)
            | Operation::ContainerRestart(payload) => payload.validate(),
            Operation::ContainerDelete(payload) => payload.validate(),
        }
    }
}

impl SiteValidatePayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Validate an ASCII DNS name rather than just excluding path separators.
        // Unicode/IDNA must be handled explicitly before constructing this protocol.
        if !valid_domain(&self.domain) {
            return Err(ValidationError::InvalidDomain);
        }

        // Validate document root
        if !self.document_root.starts_with("/srv/fusionpanel/sites/") {
            return Err(ValidationError::InvalidDocumentRoot);
        }

        if self.document_root.contains('\0') || self.document_root.contains("..") {
            return Err(ValidationError::InvalidDocumentRoot);
        }

        Ok(())
    }
}

impl SiteApplyPayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.site_id.is_nil() {
            return Err(ValidationError::InvalidSiteId);
        }
        if !valid_domain(&self.domain) || self.domain != self.domain.to_ascii_lowercase() {
            return Err(ValidationError::InvalidDomain);
        }

        match &self.spec {
            SiteSpec::Static { document_root } => {
                if !valid_document_root_for_domain(&self.domain, document_root) {
                    return Err(ValidationError::InvalidDocumentRoot);
                }
            }
            SiteSpec::ReverseProxy { upstream_url } => {
                if !valid_http_upstream_url(upstream_url) {
                    return Err(ValidationError::InvalidUpstreamUrl);
                }
            }
        }
        Ok(())
    }
}

impl SiteDeletePayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.site_id.is_nil() {
            return Err(ValidationError::InvalidSiteId);
        }
        if !valid_domain(&self.domain) || self.domain != self.domain.to_ascii_lowercase() {
            return Err(ValidationError::InvalidDomain);
        }
        Ok(())
    }
}

impl TlsApplyPayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        SiteApplyPayload {
            site_id: self.site_id,
            domain: self.domain.clone(),
            spec: self.spec.clone(),
        }
        .validate()?;

        match &self.source {
            TlsCertificateSource::CustomPem {
                certificate_pem,
                private_key_pem,
            } => {
                if !valid_certificate_pem(certificate_pem) {
                    return Err(ValidationError::InvalidCertificatePem);
                }
                if !valid_private_key_pem(private_key_pem) {
                    return Err(ValidationError::InvalidPrivateKeyPem);
                }
            }
            TlsCertificateSource::AcmeHttp01 {
                acme_directory_url,
                contact_email,
            } => {
                if !valid_http_upstream_url(acme_directory_url) {
                    return Err(ValidationError::InvalidAcmeUrl);
                }
                if let Some(email) = contact_email {
                    if !valid_contact_email(email) {
                        return Err(ValidationError::InvalidContactEmail);
                    }
                }
            }
        }
        Ok(())
    }
}

impl TlsDeletePayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        SiteApplyPayload {
            site_id: self.site_id,
            domain: self.domain.clone(),
            spec: self.spec.clone(),
        }
        .validate()
    }
}

impl DatabaseApplyPayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.database_id.is_nil() {
            return Err(ValidationError::InvalidDatabaseId);
        }
        if !valid_database_identifier(&self.name) {
            return Err(ValidationError::InvalidDatabaseIdentifier);
        }
        Ok(())
    }
}

impl DatabaseDeletePayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.database_id.is_nil() {
            return Err(ValidationError::InvalidDatabaseId);
        }
        if !valid_database_identifier(&self.name) {
            return Err(ValidationError::InvalidDatabaseIdentifier);
        }
        Ok(())
    }
}

impl DatabaseUserCreatePayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.database_id.is_nil() {
            return Err(ValidationError::InvalidDatabaseId);
        }
        if !valid_database_identifier(&self.database_name)
            || !valid_database_identifier(&self.username)
        {
            return Err(ValidationError::InvalidDatabaseIdentifier);
        }
        if self.password.is_empty()
            || self.password.len() > 256
            || self.password.chars().any(|c| c.is_control())
        {
            return Err(ValidationError::InvalidDatabasePassword);
        }
        Ok(())
    }
}

pub fn valid_database_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > 63 {
        return false;
    }
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return false;
    }
    !matches!(
        value.to_ascii_lowercase().as_str(),
        "postgres"
            | "template0"
            | "template1"
            | "mysql"
            | "information_schema"
            | "performance_schema"
            | "sys"
            | "root"
            | "fusionpanel"
            | "rdsadmin"
    )
}

impl BackupTarget {
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Self::Site {
                site_id,
                domain,
                document_root,
            } => SiteApplyPayload {
                site_id: *site_id,
                domain: domain.clone(),
                spec: SiteSpec::Static {
                    document_root: document_root.clone(),
                },
            }
            .validate(),
            Self::Database {
                database_id,
                name,
                engine,
            } => DatabaseApplyPayload {
                database_id: *database_id,
                name: name.clone(),
                engine: *engine,
            }
            .validate(),
            Self::Bundle {
                site_id,
                domain,
                document_root,
                database_id,
                database_name,
                engine,
            } => {
                SiteApplyPayload {
                    site_id: *site_id,
                    domain: domain.clone(),
                    spec: SiteSpec::Static {
                        document_root: document_root.clone(),
                    },
                }
                .validate()?;
                DatabaseApplyPayload {
                    database_id: *database_id,
                    name: database_name.clone(),
                    engine: *engine,
                }
                .validate()
            }
        }
    }
}

impl BackupDestination {
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Self::Local { base_dir } => {
                if base_dir.contains('\0')
                    || base_dir.contains("..")
                    || (!base_dir.starts_with("/var/backups/fusionpanel")
                        && !base_dir.starts_with("/srv/fusionpanel/backups")
                        && !base_dir.starts_with("/tmp/fusionpanel-backups"))
                {
                    return Err(ValidationError::InvalidBackupDestination);
                }
                Ok(())
            }
            Self::S3 {
                endpoint,
                bucket,
                region,
                access_key,
                secret_key,
                prefix,
            } => {
                if !valid_http_upstream_url(endpoint)
                    || bucket.trim().is_empty()
                    || bucket.contains('/')
                    || bucket.contains("..")
                    || region.trim().is_empty()
                    || access_key.trim().is_empty()
                    || secret_key.trim().is_empty()
                {
                    return Err(ValidationError::InvalidBackupDestination);
                }
                if let Some(prefix) = prefix {
                    if prefix.contains('\0') || prefix.contains("..") {
                        return Err(ValidationError::InvalidBackupDestination);
                    }
                }
                Ok(())
            }
        }
    }
}

fn valid_archive_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 255
        && !key.starts_with('/')
        && !key.contains('\0')
        && !key.contains("..")
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'))
}

impl BackupCreatePayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.backup_id.is_nil() || self.tenant_id.is_nil() {
            return Err(ValidationError::InvalidBackupId);
        }
        if !valid_archive_key(&self.archive_key) {
            return Err(ValidationError::InvalidBackupArchiveKey);
        }
        self.target.validate()?;
        self.destination.validate()
    }
}

fn valid_dns_owner_name(name: &str) -> bool {
    if name == "@" {
        return true;
    }
    if name.is_empty()
        || name.len() > 253
        || name.starts_with('.')
        || name.ends_with('.')
        || name.contains("..")
    {
        return false;
    }
    let labels: Vec<&str> = name.split('.').collect();
    for (idx, label) in labels.iter().enumerate() {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if *label == "*" {
            if idx != 0 {
                return false;
            }
            continue;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
        if !label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return false;
        }
    }
    true
}

impl DnsRecordSpec {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !valid_dns_owner_name(&self.name) {
            return Err(ValidationError::InvalidDnsRecord);
        }
        if self.ttl < 30 || self.ttl > 604_800 {
            return Err(ValidationError::InvalidDnsTtl);
        }
        match self.record_type {
            DnsRecordType::A => {
                if self.priority.is_some() || self.value.parse::<std::net::Ipv4Addr>().is_err() {
                    return Err(ValidationError::InvalidDnsRecord);
                }
            }
            DnsRecordType::Aaaa => {
                if self.priority.is_some() || self.value.parse::<std::net::Ipv6Addr>().is_err() {
                    return Err(ValidationError::InvalidDnsRecord);
                }
            }
            DnsRecordType::Cname => {
                let target = self.value.strip_suffix('.').unwrap_or(&self.value);
                if self.priority.is_some() || !valid_domain(target) {
                    return Err(ValidationError::InvalidDnsRecord);
                }
            }
            DnsRecordType::Mx => {
                let target = self.value.strip_suffix('.').unwrap_or(&self.value);
                if self.priority.is_none() || !valid_domain(target) {
                    return Err(ValidationError::InvalidDnsRecord);
                }
            }
            DnsRecordType::Txt => {
                if self.priority.is_some()
                    || self.value.is_empty()
                    || self.value.len() > 2048
                    || self
                        .value
                        .bytes()
                        .any(|b| b < 0x20 || b == 0x7f || b == b'\r' || b == b'\n')
                {
                    return Err(ValidationError::InvalidDnsRecord);
                }
            }
        }
        Ok(())
    }
}

impl DnsZoneApplyPayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.zone_id.is_nil() {
            return Err(ValidationError::InvalidDnsZoneId);
        }
        if !valid_domain(&self.domain) || self.domain != self.domain.to_ascii_lowercase() {
            return Err(ValidationError::InvalidDomain);
        }
        if self.serial == 0 {
            return Err(ValidationError::InvalidDnsSerial);
        }
        if self.default_ttl < 30 || self.default_ttl > 604_800 {
            return Err(ValidationError::InvalidDnsTtl);
        }
        for record in &self.records {
            record.validate()?;
        }
        Ok(())
    }
}

impl DnsZoneDeletePayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.zone_id.is_nil() {
            return Err(ValidationError::InvalidDnsZoneId);
        }
        if !valid_domain(&self.domain) || self.domain != self.domain.to_ascii_lowercase() {
            return Err(ValidationError::InvalidDomain);
        }
        Ok(())
    }
}

impl DnsProbePayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.zone_id.is_nil() {
            return Err(ValidationError::InvalidDnsZoneId);
        }
        if !valid_domain(&self.domain) || self.domain != self.domain.to_ascii_lowercase() {
            return Err(ValidationError::InvalidDomain);
        }
        if !valid_dns_owner_name(&self.record_name) {
            return Err(ValidationError::InvalidDnsRecord);
        }
        if let Some(substr) = &self.expected_substring {
            if substr.is_empty()
                || substr.len() > 512
                || substr.bytes().any(|b| b < 0x20 || b == 0x7f)
            {
                return Err(ValidationError::InvalidDnsRecord);
            }
        }
        Ok(())
    }
}

fn valid_cron_schedule(schedule: &str) -> bool {
    let fields: Vec<&str> = schedule.split_whitespace().collect();
    if fields.len() != 5 {
        return false;
    }
    fields.iter().all(|field| {
        !field.is_empty()
            && field.len() <= 32
            && field
                .bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'*' | b'/' | b'-' | b','))
    })
}

fn valid_cron_command(command: &str) -> bool {
    let trimmed = command.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 512
        && !trimmed
            .bytes()
            .any(|b| b < 0x20 || b == 0x7f || b == b'\r' || b == b'\n')
}

impl CronJobSpec {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.cron_id.is_nil() {
            return Err(ValidationError::InvalidCronJobId);
        }
        if self.name.trim().is_empty()
            || self.name.len() > 64
            || !self
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b' '))
        {
            return Err(ValidationError::InvalidCronCommand);
        }
        if !valid_cron_schedule(&self.schedule) {
            return Err(ValidationError::InvalidCronSchedule);
        }
        if !valid_cron_command(&self.command) {
            return Err(ValidationError::InvalidCronCommand);
        }
        Ok(())
    }
}

impl CronApplyPayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.tenant_id.is_nil() || self.cron_id.is_nil() {
            return Err(ValidationError::InvalidCronJobId);
        }
        for job in &self.jobs {
            job.validate()?;
        }
        Ok(())
    }
}

impl CronDeletePayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.tenant_id.is_nil() || self.cron_id.is_nil() {
            return Err(ValidationError::InvalidCronJobId);
        }
        for job in &self.jobs {
            job.validate()?;
        }
        Ok(())
    }
}

impl CronRunNowPayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.tenant_id.is_nil() || self.cron_id.is_nil() {
            return Err(ValidationError::InvalidCronJobId);
        }
        if !valid_cron_command(&self.command) {
            return Err(ValidationError::InvalidCronCommand);
        }
        Ok(())
    }
}

impl BackupRestorePayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.backup_id.is_nil() || self.tenant_id.is_nil() {
            return Err(ValidationError::InvalidBackupId);
        }
        if !valid_archive_key(&self.archive_key) {
            return Err(ValidationError::InvalidBackupArchiveKey);
        }
        if let Some(sha) = &self.expected_sha256 {
            if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(ValidationError::InvalidBackupArchiveKey);
            }
        }
        self.target.validate()?;
        self.destination.validate()
    }
}

fn valid_certificate_pem(pem: &str) -> bool {
    let trimmed = pem.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 64 * 1024
        && trimmed.contains("-----BEGIN CERTIFICATE-----")
        && trimmed.contains("-----END CERTIFICATE-----")
}

fn valid_private_key_pem(pem: &str) -> bool {
    let trimmed = pem.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 32 * 1024
        && trimmed.contains("-----BEGIN ")
        && trimmed.contains("PRIVATE KEY-----")
        && trimmed.contains("-----END ")
}

fn valid_contact_email(email: &str) -> bool {
    let trimmed = email.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 254
        && !trimmed.chars().any(|c| c.is_whitespace() || c.is_control())
        && matches!(trimmed.split_once('@'), Some((local, domain)) if !local.is_empty() && domain.contains('.'))
}

fn valid_document_root_for_domain(domain: &str, document_root: &str) -> bool {
    if document_root.len() > 4096
        || document_root.contains('\\')
        || document_root.chars().any(char::is_control)
        || !document_root.starts_with("/srv/fusionpanel/sites/")
        || !document_root
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        return false;
    }

    let mut parts = document_root.split('/');
    if parts.next() != Some("")
        || parts.next() != Some("srv")
        || parts.next() != Some("fusionpanel")
        || parts.next() != Some("sites")
        || parts.next() != Some(domain)
    {
        return false;
    }
    parts.all(|part| !part.is_empty() && part != "." && part != "..")
}

fn valid_http_upstream_url(raw_url: &str) -> bool {
    if raw_url.is_empty()
        || raw_url.len() > 2048
        || raw_url.chars().any(|character| {
            character.is_whitespace()
                || character.is_control()
                || matches!(
                    character,
                    '\\' | '"' | '\'' | ';' | '{' | '}' | '$' | '`' | '?' | '#'
                )
        })
    {
        return false;
    }

    let Some((scheme, remainder)) = raw_url.split_once("://") else {
        return false;
    };
    if !matches!(scheme, "http" | "https") || remainder.is_empty() {
        return false;
    }

    let authority = remainder.split('/').next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return false;
    }

    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        let Some((host, suffix)) = bracketed.split_once(']') else {
            return false;
        };
        if host.parse::<std::net::Ipv6Addr>().is_err() {
            return false;
        }
        let port = if suffix.is_empty() {
            None
        } else if let Some(port) = suffix.strip_prefix(':') {
            Some(port)
        } else {
            return false;
        };
        (Some(host), port)
    } else {
        if authority.contains('[') || authority.contains(']') {
            return false;
        }
        match authority.rsplit_once(':') {
            Some((host, port)) if !host.contains(':') => (Some(host), Some(port)),
            Some(_) => return false,
            None => (Some(authority), None),
        }
    };

    let Some(host) = host else {
        return false;
    };
    if host.is_empty() || (host.parse::<std::net::IpAddr>().is_err() && !valid_domain(host)) {
        return false;
    }
    if let Some(port) = port {
        if port.is_empty()
            || !port.bytes().all(|byte| byte.is_ascii_digit())
            || !port.parse::<u16>().is_ok_and(|value| value != 0)
        {
            return false;
        }
    }

    let path_start = authority.len();
    let path = &remainder[path_start..];
    if path.is_empty() {
        return true;
    }
    if !path.starts_with('/')
        || Path::new(path)
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return false;
    }

    let bytes = path.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return false;
            }
            index += 3;
            continue;
        }
        if !(byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'/' | b'-'
                    | b'.'
                    | b'_'
                    | b'~'
                    | b'!'
                    | b'&'
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b'='
                    | b':'
                    | b'@'
            ))
        {
            return false;
        }
        index += 1;
    }
    true
}

fn valid_domain(domain: &str) -> bool {
    if domain.is_empty()
        || domain.len() > 253
        || !domain.is_ascii()
        || domain.parse::<std::net::IpAddr>().is_ok()
    {
        return false;
    }

    domain.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

pub const MAX_CONTAINER_PORTS: usize = 32;
pub const MAX_CONTAINER_ENV_VARS: usize = 64;
pub const MAX_CONTAINER_COMMAND_ARGS: usize = 64;

pub fn valid_container_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 63 || !name.is_ascii() {
        return false;
    }
    let bytes = name.as_bytes();
    if !(bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit()) {
        return false;
    }
    if !(bytes[bytes.len() - 1].is_ascii_lowercase() || bytes[bytes.len() - 1].is_ascii_digit()) {
        return false;
    }
    bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

pub fn valid_pinned_image_ref(image: &str) -> bool {
    if image.is_empty()
        || image.len() > 512
        || !image.is_ascii()
        || image.bytes().any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        || image.contains("..")
        || image.contains("//")
    {
        return false;
    }

    let Some((repo_and_tag, digest)) = image.split_once("@sha256:") else {
        return false;
    };
    if digest.len() != 64 || !digest.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return false;
    }
    if repo_and_tag.is_empty() || repo_and_tag.contains('@') {
        return false;
    }

    let mut segments = repo_and_tag.split('/');
    let Some(first) = segments.next() else {
        return false;
    };
    if first.is_empty() {
        return false;
    }

    // Validate all path segments; only the first segment may carry a registry :port,
    // and only the last segment may carry a :tag.
    let all_segments: Vec<&str> = std::iter::once(first).chain(segments).collect();
    for (idx, seg) in all_segments.iter().enumerate() {
        if seg.is_empty() {
            return false;
        }
        let is_first = idx == 0;
        let is_last = idx + 1 == all_segments.len();
        if let Some((left, right)) = seg.split_once(':') {
            if left.is_empty() || right.is_empty() || right.contains(':') {
                return false;
            }
            if !valid_image_name_segment(left) {
                return false;
            }
            if is_last && !is_first {
                if !valid_image_tag(right) {
                    return false;
                }
            } else if is_first && !is_last {
                // Registry port (digits) or tag if single-segment
                if !right.bytes().all(|b| b.is_ascii_digit()) {
                    return false;
                }
            } else if is_first && is_last {
                if !valid_image_tag(right) {
                    return false;
                }
            } else {
                return false;
            }
        } else if !valid_image_name_segment(seg) {
            return false;
        }
    }

    true
}

fn valid_image_name_segment(seg: &str) -> bool {
    !seg.is_empty()
        && !seg.starts_with('.')
        && !seg.starts_with('-')
        && !seg.ends_with('.')
        && !seg.ends_with('-')
        && seg
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'))
}

fn valid_image_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 128
        && !tag.starts_with('.')
        && !tag.starts_with('-')
        && tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

pub fn normalize_container_ports(
    ports: &[ContainerPortMapping],
) -> Result<Vec<ContainerPortMapping>, ValidationError> {
    if ports.len() > MAX_CONTAINER_PORTS {
        return Err(ValidationError::InvalidContainerPorts);
    }
    let mut seen_host_bindings = BTreeSet::new();
    let mut normalized = Vec::with_capacity(ports.len());
    for port in ports {
        if port.host_port == 0 || port.container_port == 0 {
            return Err(ValidationError::InvalidContainerPorts);
        }
        if !seen_host_bindings.insert((port.host_port, port.protocol)) {
            return Err(ValidationError::InvalidContainerPorts);
        }
        normalized.push(*port);
    }
    normalized.sort();
    Ok(normalized)
}

impl ContainerApplyPayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.container_id.is_nil() {
            return Err(ValidationError::InvalidContainerId);
        }
        if self.tenant_id.is_nil() {
            return Err(ValidationError::InvalidTenantId);
        }
        if !valid_container_name(&self.name) {
            return Err(ValidationError::InvalidContainerName);
        }
        if !valid_pinned_image_ref(&self.image) {
            return Err(ValidationError::InvalidContainerImage);
        }
        normalize_container_ports(&self.ports)?;
        self.resources.validate()?;
        if self.env.len() > MAX_CONTAINER_ENV_VARS {
            return Err(ValidationError::InvalidContainerEnv);
        }
        for (k, v) in &self.env {
            if k.is_empty()
                || k.len() > 128
                || !k.is_ascii()
                || k.bytes().any(|b| b == b'\0' || b == b'\n' || b == b'\r' || b == b'=')
                || v.len() > 4096
                || v.contains('\0')
            {
                return Err(ValidationError::InvalidContainerEnv);
            }
            let mut chars = k.bytes();
            let Some(first) = chars.next() else {
                return Err(ValidationError::InvalidContainerEnv);
            };
            if !(first.is_ascii_alphabetic() || first == b'_')
                || !chars.all(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                return Err(ValidationError::InvalidContainerEnv);
            }
        }
        if self.command.len() > MAX_CONTAINER_COMMAND_ARGS {
            return Err(ValidationError::InvalidContainerCommand);
        }
        for arg in &self.command {
            if arg.is_empty() || arg.len() > 1024 || arg.contains('\0') {
                return Err(ValidationError::InvalidContainerCommand);
            }
        }
        Ok(())
    }
}

impl ContainerLifecyclePayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.container_id.is_nil() {
            return Err(ValidationError::InvalidContainerId);
        }
        if self.tenant_id.is_nil() {
            return Err(ValidationError::InvalidTenantId);
        }
        if !valid_container_name(&self.name) {
            return Err(ValidationError::InvalidContainerName);
        }
        Ok(())
    }
}

impl ContainerDeletePayload {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.container_id.is_nil() {
            return Err(ValidationError::InvalidContainerId);
        }
        if self.tenant_id.is_nil() {
            return Err(ValidationError::InvalidTenantId);
        }
        if !valid_container_name(&self.name) {
            return Err(ValidationError::InvalidContainerName);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_system_info_roundtrip() {
        let req_json = json!({
            "version": 1,
            "request_id": "00000000-0000-0000-0000-000000000000",
            "operation": "system.info",
            "payload": {}
        });

        let req: Request = serde_json::from_value(req_json.clone()).unwrap();
        assert_eq!(req.version, 1);
        assert!(matches!(req.operation, Operation::SystemInfo(_)));

        assert_eq!(serde_json::to_value(&req).unwrap(), req_json);
    }

    #[test]
    fn test_site_validate_roundtrip() {
        let req_json = json!({
            "version": 1,
            "request_id": "00000000-0000-0000-0000-000000000000",
            "operation": "site.validate",
            "payload": {
                "domain": "example.com",
                "document_root": "/srv/fusionpanel/sites/example.com"
            }
        });

        let req: Request = serde_json::from_value(req_json.clone()).unwrap();
        assert_eq!(req.version, 1);
        if let Operation::SiteValidate(p) = &req.operation {
            assert_eq!(p.domain, "example.com");
            assert_eq!(p.document_root, "/srv/fusionpanel/sites/example.com");
        } else {
            panic!("Wrong operation");
        }

        assert_eq!(serde_json::to_value(&req).unwrap(), req_json);
    }

    #[test]
    fn test_site_apply_static_and_delete_roundtrip() {
        let site_id = "00000000-0000-4000-8000-000000000001";
        let apply_json = json!({
            "version": 1,
            "request_id": "00000000-0000-4000-8000-000000000002",
            "operation": "site.apply",
            "payload": {
                "site_id": site_id,
                "domain": "example.test",
                "spec": {
                    "kind": "static",
                    "document_root": "/srv/fusionpanel/sites/example.test"
                }
            }
        });
        let apply: Request = serde_json::from_value(apply_json.clone()).unwrap();
        assert_eq!(apply.validate(), Ok(()));
        assert!(matches!(apply.operation, Operation::SiteApply(_)));
        assert_eq!(serde_json::to_value(&apply).unwrap(), apply_json);

        let delete_json = json!({
            "version": 1,
            "request_id": "00000000-0000-4000-8000-000000000003",
            "operation": "site.delete",
            "payload": { "site_id": site_id, "domain": "example.test" }
        });
        let delete: Request = serde_json::from_value(delete_json.clone()).unwrap();
        assert_eq!(delete.validate(), Ok(()));
        assert!(matches!(delete.operation, Operation::SiteDelete(_)));
        assert_eq!(serde_json::to_value(&delete).unwrap(), delete_json);
    }

    #[test]
    fn test_site_apply_reverse_proxy_roundtrip() {
        let value = json!({
            "version": 1,
            "request_id": "00000000-0000-4000-8000-000000000002",
            "operation": "site.apply",
            "payload": {
                "site_id": "00000000-0000-4000-8000-000000000001",
                "domain": "proxy.example.test",
                "spec": {
                    "kind": "reverse_proxy",
                    "upstream_url": "https://127.0.0.1:8443/api"
                }
            }
        });
        let request: Request = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(request.validate(), Ok(()));
        assert_eq!(serde_json::to_value(request).unwrap(), value);
    }

    #[test]
    fn test_site_operations_reject_unknown_or_malformed_envelopes() {
        for value in [
            json!({
                "version": 1,
                "request_id": Uuid::nil(),
                "operation": "site.apply",
                "payload": {
                    "site_id": "00000000-0000-4000-8000-000000000001",
                    "domain": "example.test",
                    "spec": { "kind": "php", "version": "8.2" }
                }
            }),
            json!({
                "version": 1,
                "request_id": Uuid::nil(),
                "operation": "site.delete",
                "payload": {
                    "site_id": "00000000-0000-4000-8000-000000000001",
                    "domain": "example.test",
                    "force": true
                }
            }),
            json!({
                "version": 1,
                "request_id": Uuid::nil(),
                "operation": "site.apply",
                "payload": {
                    "site_id": "00000000-0000-4000-8000-000000000001",
                    "domain": "example.test",
                    "spec": {
                        "kind": "static",
                        "document_root": "/srv/fusionpanel/sites/example.test",
                        "extra": "rejected"
                    }
                }
            }),
        ] {
            assert!(serde_json::from_value::<Request>(value).is_err());
        }

        let unsupported: Request = serde_json::from_value(json!({
            "version": 2,
            "request_id": Uuid::nil(),
            "operation": "site.delete",
            "payload": {
                "site_id": "00000000-0000-4000-8000-000000000001",
                "domain": "example.test"
            }
        }))
        .unwrap();
        assert_eq!(
            unsupported.validate(),
            Err(ValidationError::UnsupportedVersion)
        );
    }

    #[test]
    fn test_site_apply_rejects_invalid_id_domain_and_document_root() {
        let mut payload = SiteApplyPayload {
            site_id: Uuid::nil(),
            domain: "example.test".to_string(),
            spec: SiteSpec::Static {
                document_root: "/srv/fusionpanel/sites/example.test".to_string(),
            },
        };
        assert_eq!(payload.validate(), Err(ValidationError::InvalidSiteId));

        payload.site_id = Uuid::new_v4();
        payload.domain = "bad.example/../../other".to_string();
        assert_eq!(payload.validate(), Err(ValidationError::InvalidDomain));

        payload.domain = "example.test".to_string();
        for document_root in [
            "/srv/fusionpanel/sites/other.test",
            "/srv/fusionpanel/sites/example.test/../other",
            "/srv/fusionpanel/sites/example.test/./private",
            "/srv/fusionpanel/sites/example.test/",
            "/tmp/example.test",
        ] {
            payload.spec = SiteSpec::Static {
                document_root: document_root.to_string(),
            };
            assert_eq!(
                payload.validate(),
                Err(ValidationError::InvalidDocumentRoot),
                "accepted {document_root:?}"
            );
        }
    }

    #[test]
    fn test_site_apply_accepts_only_absolute_safe_http_upstreams() {
        for upstream_url in [
            "http://127.0.0.1:8080",
            "https://[::1]:8443/health",
            "http://service.internal:9000/api/v1",
            "http://unresolvable.invalid",
        ] {
            let payload = SiteApplyPayload {
                site_id: Uuid::new_v4(),
                domain: "proxy.example.test".to_string(),
                spec: SiteSpec::ReverseProxy {
                    upstream_url: upstream_url.to_string(),
                },
            };
            assert_eq!(payload.validate(), Ok(()), "rejected {upstream_url:?}");
        }

        for upstream_url in [
            "ftp://service.internal",
            "http://",
            "http:///missing-host",
            "http://user@service.internal",
            "http://service.internal:0",
            "http://service.internal:65536",
            "http://service.internal?secret=1",
            "http://service.internal#fragment",
            "http://service.internal;include=/tmp/x",
            "http://service.internal/\ninclude /tmp/x",
            "http://[not-ipv6]:8080",
            "http://2001:db8::1:8080",
        ] {
            let payload = SiteApplyPayload {
                site_id: Uuid::new_v4(),
                domain: "proxy.example.test".to_string(),
                spec: SiteSpec::ReverseProxy {
                    upstream_url: upstream_url.to_string(),
                },
            };
            assert_eq!(
                payload.validate(),
                Err(ValidationError::InvalidUpstreamUrl),
                "accepted {upstream_url:?}"
            );
        }
    }

    #[test]
    fn test_delete_rejects_nil_id_and_invalid_domain() {
        let mut payload = SiteDeletePayload {
            site_id: Uuid::nil(),
            domain: "example.test".to_string(),
        };
        assert_eq!(payload.validate(), Err(ValidationError::InvalidSiteId));
        payload.site_id = Uuid::new_v4();
        payload.domain = "127.0.0.1".to_string();
        assert_eq!(payload.validate(), Err(ValidationError::InvalidDomain));
    }

    #[test]
    fn test_unknown_fields_rejected_in_request() {
        let req_json = json!({
            "version": 1,
            "request_id": "00000000-0000-0000-0000-000000000000",
            "operation": "system.info",
            "payload": {},
            "extra_field": "bad"
        });

        assert!(serde_json::from_value::<Request>(req_json).is_err());
    }

    #[test]
    fn test_unknown_payload_fields_are_rejected() {
        for value in [
            json!({"version": 1, "request_id": Uuid::nil(), "operation": "system.info", "payload": {"extra": true}}),
            json!({"version": 1, "request_id": Uuid::nil(), "operation": "site.validate", "payload": {"domain": "example.com", "document_root": "/srv/fusionpanel/sites/example.com", "extra": true}}),
        ] {
            assert!(serde_json::from_value::<Request>(value).is_err());
        }
    }

    #[test]
    fn test_unknown_response_and_error_fields_are_rejected() {
        for value in [
            json!({"version": 1, "request_id": Uuid::nil(), "result": {}, "extra": true}),
            json!({"version": 1, "request_id": Uuid::nil(), "error": {"code": "invalid", "message": "bad", "extra": true}}),
            json!({"version": 1, "request_id": Uuid::nil()}),
        ] {
            assert!(serde_json::from_value::<Response>(value).is_err());
        }
    }

    #[test]
    fn test_unknown_operation_rejected() {
        let req_json = json!({
            "version": 1,
            "request_id": "00000000-0000-0000-0000-000000000000",
            "operation": "system.exec",
            "payload": {}
        });

        assert!(serde_json::from_value::<Request>(req_json).is_err());
    }

    #[test]
    fn test_validation_unsupported_version() {
        let req = Request {
            version: 2,
            request_id: Uuid::nil(),
            operation: Operation::SystemInfo(SystemInfoPayload {}),
        };
        assert_eq!(req.validate(), Err(ValidationError::UnsupportedVersion));
    }

    #[test]
    fn test_validation_invalid_domain() {
        let p = SiteValidatePayload {
            domain: "example.com/bad".to_string(),
            document_root: "/srv/fusionpanel/sites/example.com".to_string(),
        };
        assert_eq!(p.validate(), Err(ValidationError::InvalidDomain));

        let p = SiteValidatePayload {
            domain: "../example.com".to_string(),
            document_root: "/srv/fusionpanel/sites/example.com".to_string(),
        };
        assert_eq!(p.validate(), Err(ValidationError::InvalidDomain));
    }

    #[test]
    fn test_dns_validation_rejects_malformed_names() {
        let overlong_label = format!("{}.example", "a".repeat(64));
        let overlong_name = format!(
            "{}.{}.{}.{}",
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(63)
        );
        for domain in [
            "",
            "invalid domain.example",
            "-bad.example",
            "bad-.example",
            "bad_.example",
            "éxample.com",
            "127.0.0.1",
            "::1",
            "example.com\nother",
            "example.com\r",
            "example.com\t",
            ".example.com",
            "example.com.",
            "example..com",
            "example/com",
            "example\\com",
            "example\0com",
            "example;include.conf",
            overlong_label.as_str(),
            overlong_name.as_str(),
        ] {
            let payload = SiteValidatePayload {
                domain: domain.to_string(),
                document_root: "/srv/fusionpanel/sites/example.com".to_string(),
            };
            assert_eq!(
                payload.validate(),
                Err(ValidationError::InvalidDomain),
                "accepted {domain:?}"
            );
        }
    }

    #[test]
    fn test_dns_validation_preserves_valid_names_and_label_boundaries() {
        let max_label = format!("{}.example", "a".repeat(63));
        let max_name = format!(
            "{}.{}.{}.{}",
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(61)
        );
        assert_eq!(max_name.len(), 253);
        for domain in [
            "example.com",
            "Sub.Example.COM",
            "my-site.example",
            "123.example",
            max_label.as_str(),
            max_name.as_str(),
        ] {
            let payload = SiteValidatePayload {
                domain: domain.to_string(),
                document_root: "/srv/fusionpanel/sites/example.com".to_string(),
            };
            assert_eq!(payload.validate(), Ok(()), "rejected {domain:?}");
        }
    }

    #[test]
    fn test_validation_invalid_document_root() {
        let p = SiteValidatePayload {
            domain: "example.com".to_string(),
            document_root: "/tmp/hacked".to_string(),
        };
        assert_eq!(p.validate(), Err(ValidationError::InvalidDocumentRoot));

        let p = SiteValidatePayload {
            domain: "example.com".to_string(),
            document_root: "/srv/fusionpanel/sites/example.com/../../hacked".to_string(),
        };
        assert_eq!(p.validate(), Err(ValidationError::InvalidDocumentRoot));

        let p = SiteValidatePayload {
            domain: "example.com".to_string(),
            document_root: "/srv/fusionpanel/sites/example.com\0".to_string(),
        };
        assert_eq!(p.validate(), Err(ValidationError::InvalidDocumentRoot));
    }

    #[test]
    fn test_response_result_roundtrip() {
        let res_json = json!({
            "version": 1,
            "request_id": "00000000-0000-0000-0000-000000000000",
            "result": { "status": "ok" }
        });

        let res: Response = serde_json::from_value(res_json.clone()).unwrap();
        assert_eq!(res.version, 1);
        if let ResponseResult::Result(v) = &res.result {
            assert_eq!(v, &json!({"status": "ok"}));
        } else {
            panic!("Wrong result");
        }

        assert_eq!(serde_json::to_value(&res).unwrap(), res_json);
    }

    #[test]
    fn test_response_error_roundtrip() {
        let res_json = json!({
            "version": 1,
            "request_id": "00000000-0000-0000-0000-000000000000",
            "error": {
                "code": "invalid",
                "message": "bad input"
            }
        });

        let res: Response = serde_json::from_value(res_json.clone()).unwrap();
        assert_eq!(res.version, 1);
        if let ResponseResult::Error(e) = &res.result {
            assert_eq!(e.code, "invalid");
            assert_eq!(e.message, "bad input");
        } else {
            panic!("Wrong result");
        }

        assert_eq!(serde_json::to_value(&res).unwrap(), res_json);
    }

    #[test]
    fn test_response_mutually_exclusive() {
        let res_json = json!({
            "version": 1,
            "request_id": "00000000-0000-0000-0000-000000000000",
            "result": { "status": "ok" },
            "error": {
                "code": "invalid",
                "message": "bad input"
            }
        });

        assert!(serde_json::from_value::<Response>(res_json).is_err());
    }

    #[test]
    fn test_tls_apply_and_delete_validation() {
        let valid_custom = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::TlsApply(TlsApplyPayload {
                site_id: Uuid::new_v4(),
                domain: "example.com".to_string(),
                spec: SiteSpec::Static {
                    document_root: "/srv/fusionpanel/sites/example.com".to_string(),
                },
                source: TlsCertificateSource::CustomPem {
                    certificate_pem: "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----"
                        .to_string(),
                    private_key_pem: "-----BEGIN PRIVATE KEY-----\nMIIE\n-----END PRIVATE KEY-----"
                        .to_string(),
                },
            }),
        };
        assert!(valid_custom.validate().is_ok());

        let invalid_acme = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::TlsApply(TlsApplyPayload {
                site_id: Uuid::new_v4(),
                domain: "example.com".to_string(),
                spec: SiteSpec::ReverseProxy {
                    upstream_url: "http://127.0.0.1:8080/".to_string(),
                },
                source: TlsCertificateSource::AcmeHttp01 {
                    acme_directory_url: "ftp://bad.example/dir".to_string(),
                    contact_email: Some("ops@example.com".to_string()),
                },
            }),
        };
        assert_eq!(
            invalid_acme.validate(),
            Err(ValidationError::InvalidAcmeUrl)
        );

        let valid_delete = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::TlsDelete(TlsDeletePayload {
                site_id: Uuid::new_v4(),
                domain: "example.com".to_string(),
                spec: SiteSpec::Static {
                    document_root: "/srv/fusionpanel/sites/example.com".to_string(),
                },
            }),
        };
        assert!(valid_delete.validate().is_ok());
    }

    #[test]
    fn test_database_apply_delete_and_user_validation() {
        let valid_apply = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::DatabaseApply(DatabaseApplyPayload {
                database_id: Uuid::new_v4(),
                name: "tenant_app_db".to_string(),
                engine: DatabaseEngineKind::Postgresql,
            }),
        };
        assert!(valid_apply.validate().is_ok());

        let reserved_name = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::DatabaseApply(DatabaseApplyPayload {
                database_id: Uuid::new_v4(),
                name: "postgres".to_string(),
                engine: DatabaseEngineKind::Postgresql,
            }),
        };
        assert_eq!(
            reserved_name.validate(),
            Err(ValidationError::InvalidDatabaseIdentifier)
        );

        let invalid_chars = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::DatabaseDelete(DatabaseDeletePayload {
                database_id: Uuid::new_v4(),
                name: "bad;drop".to_string(),
                engine: DatabaseEngineKind::Mariadb,
            }),
        };
        assert_eq!(
            invalid_chars.validate(),
            Err(ValidationError::InvalidDatabaseIdentifier)
        );

        let valid_user = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::DatabaseUserCreate(DatabaseUserCreatePayload {
                database_id: Uuid::new_v4(),
                database_name: "tenant_app_db".to_string(),
                engine: DatabaseEngineKind::Mariadb,
                username: "tenant_user".to_string(),
                password: "S3curePassword!123".to_string(),
            }),
        };
        assert!(valid_user.validate().is_ok());

        let empty_password = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::DatabaseUserCreate(DatabaseUserCreatePayload {
                database_id: Uuid::new_v4(),
                database_name: "tenant_app_db".to_string(),
                engine: DatabaseEngineKind::Mariadb,
                username: "tenant_user".to_string(),
                password: "".to_string(),
            }),
        };
        assert_eq!(
            empty_password.validate(),
            Err(ValidationError::InvalidDatabasePassword)
        );
    }

    #[test]
    fn test_backup_create_and_restore_validation() {
        let valid_create = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::BackupCreate(BackupCreatePayload {
                backup_id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                archive_key: "tenant-1/backup-1.tar.gz".to_string(),
                target: BackupTarget::Bundle {
                    site_id: Uuid::new_v4(),
                    domain: "app.local".to_string(),
                    document_root: "/srv/fusionpanel/sites/app.local".to_string(),
                    database_id: Uuid::new_v4(),
                    database_name: "app_db".to_string(),
                    engine: DatabaseEngineKind::Postgresql,
                },
                destination: BackupDestination::S3 {
                    endpoint: "http://127.0.0.1:9000".to_string(),
                    bucket: "fusionpanel-backups".to_string(),
                    region: "us-east-1".to_string(),
                    access_key: "minioadmin".to_string(),
                    secret_key: "minioadmin".to_string(),
                    prefix: Some("sprint6".to_string()),
                },
            }),
        };
        assert!(valid_create.validate().is_ok());

        let bad_key = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::BackupRestore(BackupRestorePayload {
                backup_id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                archive_key: "../escape.tar.gz".to_string(),
                target: BackupTarget::Site {
                    site_id: Uuid::new_v4(),
                    domain: "app.local".to_string(),
                    document_root: "/srv/fusionpanel/sites/app.local".to_string(),
                },
                destination: BackupDestination::Local {
                    base_dir: "/var/backups/fusionpanel".to_string(),
                },
                expected_sha256: None,
            }),
        };
        assert_eq!(
            bad_key.validate(),
            Err(ValidationError::InvalidBackupArchiveKey)
        );
    }

    #[test]
    fn test_dns_zone_and_probe_validation() {
        let zone_id = Uuid::new_v4();
        let valid_apply = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::DnsZoneApply(DnsZoneApplyPayload {
                zone_id,
                domain: "example.com".to_string(),
                serial: 2026111701,
                default_ttl: 300,
                records: vec![
                    DnsRecordSpec {
                        name: "@".to_string(),
                        record_type: DnsRecordType::A,
                        value: "203.0.113.10".to_string(),
                        ttl: 300,
                        priority: None,
                    },
                    DnsRecordSpec {
                        name: "default._domainkey".to_string(),
                        record_type: DnsRecordType::Txt,
                        value: "v=DKIM1; k=rsa; p=MIIBIjANBgkqhkiG".to_string(),
                        ttl: 300,
                        priority: None,
                    },
                    DnsRecordSpec {
                        name: "@".to_string(),
                        record_type: DnsRecordType::Mx,
                        value: "mail.example.com".to_string(),
                        ttl: 300,
                        priority: Some(10),
                    },
                ],
            }),
        };
        assert!(valid_apply.validate().is_ok());

        let bad_mx = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::DnsZoneApply(DnsZoneApplyPayload {
                zone_id,
                domain: "example.com".to_string(),
                serial: 1,
                default_ttl: 300,
                records: vec![DnsRecordSpec {
                    name: "@".to_string(),
                    record_type: DnsRecordType::Mx,
                    value: "mail.example.com".to_string(),
                    ttl: 300,
                    priority: None,
                }],
            }),
        };
        assert_eq!(bad_mx.validate(), Err(ValidationError::InvalidDnsRecord));

        let valid_probe = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::DnsProbe(DnsProbePayload {
                zone_id,
                domain: "example.com".to_string(),
                record_name: "default._domainkey".to_string(),
                record_type: DnsRecordType::Txt,
                expected_substring: Some("v=DKIM1;".to_string()),
            }),
        };
        assert!(valid_probe.validate().is_ok());
    }

    #[test]
    fn test_cron_payload_validation() {
        let tenant_id = Uuid::new_v4();
        let cron_id = Uuid::new_v4();
        let valid = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::CronApply(CronApplyPayload {
                tenant_id,
                cron_id,
                jobs: vec![CronJobSpec {
                    cron_id,
                    name: "heartbeat".to_string(),
                    schedule: "*/5 * * * *".to_string(),
                    command: "echo ok".to_string(),
                    enabled: true,
                }],
            }),
        };
        assert!(valid.validate().is_ok());

        let bad_sched = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::CronApply(CronApplyPayload {
                tenant_id,
                cron_id,
                jobs: vec![CronJobSpec {
                    cron_id,
                    name: "bad".to_string(),
                    schedule: "* * *".to_string(),
                    command: "echo ok".to_string(),
                    enabled: true,
                }],
            }),
        };
        assert_eq!(
            bad_sched.validate(),
            Err(ValidationError::InvalidCronSchedule)
        );
    }

    #[test]
    fn test_container_pinned_image_and_port_normalization() {
        let digest = "a".repeat(64);
        assert!(valid_pinned_image_ref(&format!("nginx@sha256:{digest}")));
        assert!(valid_pinned_image_ref(&format!(
            "ghcr.io/fusionpanel/app:1.2.3@sha256:{digest}"
        )));
        assert!(valid_pinned_image_ref(&format!(
            "localhost:5000/my-app@sha256:{digest}"
        )));

        // Unpinned or malformed images must be rejected
        assert!(!valid_pinned_image_ref("nginx"));
        assert!(!valid_pinned_image_ref("nginx:latest"));
        assert!(!valid_pinned_image_ref("ghcr.io/fusionpanel/app:1.2.3"));
        assert!(!valid_pinned_image_ref("nginx@sha256:short"));
        assert!(!valid_pinned_image_ref(&format!(
            "nginx@sha256:{}",
            "A".repeat(64)
        )));

        let ports = vec![
            ContainerPortMapping {
                host_port: 8443,
                container_port: 443,
                protocol: ContainerPortProtocol::Tcp,
            },
            ContainerPortMapping {
                host_port: 8080,
                container_port: 80,
                protocol: ContainerPortProtocol::Udp,
            },
            ContainerPortMapping {
                host_port: 8080,
                container_port: 80,
                protocol: ContainerPortProtocol::Tcp,
            },
        ];
        let normalized = normalize_container_ports(&ports).unwrap();
        assert_eq!(normalized[0].host_port, 8080);
        assert_eq!(normalized[0].protocol, ContainerPortProtocol::Tcp);
        assert_eq!(normalized[1].host_port, 8080);
        assert_eq!(normalized[1].protocol, ContainerPortProtocol::Udp);
        assert_eq!(normalized[2].host_port, 8443);

        // Duplicate (host_port, protocol) rejected
        let dup_ports = vec![
            ContainerPortMapping {
                host_port: 8080,
                container_port: 80,
                protocol: ContainerPortProtocol::Tcp,
            },
            ContainerPortMapping {
                host_port: 8080,
                container_port: 81,
                protocol: ContainerPortProtocol::Tcp,
            },
        ];
        assert_eq!(
            normalize_container_ports(&dup_ports),
            Err(ValidationError::InvalidContainerPorts)
        );
    }

    #[test]
    fn test_container_payload_validation_and_roundtrip() {
        let digest = "0123456789abcdef".repeat(4);
        let container_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let req = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::ContainerApply(ContainerApplyPayload {
                container_id,
                tenant_id,
                name: "web-app-1".to_string(),
                image: format!("ghcr.io/org/web:1.0@sha256:{digest}"),
                ports: vec![ContainerPortMapping {
                    host_port: 8080,
                    container_port: 80,
                    protocol: ContainerPortProtocol::Tcp,
                }],
                resources: ContainerResourceLimits {
                    cpu_millicores: 500,
                    memory_bytes: 256 * 1024 * 1024,
                    pids_limit: Some(128),
                },
                env: BTreeMap::from([("APP_ENV".to_string(), "production".to_string())]),
                command: vec!["/bin/server".to_string()],
                restart_policy: ContainerRestartPolicy::UnlessStopped,
            }),
        };
        assert!(req.validate().is_ok());
        let encoded = serde_json::to_string(&req).unwrap();
        let decoded: Request = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, req);

        // Out-of-bounds resources rejected
        let mut bad_res = req.clone();
        if let Operation::ContainerApply(ref mut p) = bad_res.operation {
            p.resources.cpu_millicores = 0;
        }
        assert_eq!(
            bad_res.validate(),
            Err(ValidationError::InvalidContainerResources)
        );

        // Unpinned image rejected
        let mut bad_img = req;
        if let Operation::ContainerApply(ref mut p) = bad_img.operation {
            p.image = "nginx:latest".to_string();
        }
        assert_eq!(
            bad_img.validate(),
            Err(ValidationError::InvalidContainerImage)
        );
    }
}
