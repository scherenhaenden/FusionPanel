#![forbid(unsafe_code)]

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const DEFAULT_MANAGED_ROOT: &str = "/etc/nginx/fusionpanel";
const DEFAULT_DOCUMENT_ROOT: &str = "/srv/fusionpanel/sites";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedSiteSpec {
    Static { document_root: String },
    ReverseProxy { upstream_url: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedSiteError {
    InvalidInput,
    UnsafePath,
    NotManaged,
    Io,
    CandidateRejected,
    ActiveConfigRejected,
    ReloadFailed,
    RollbackFailed,
    HelperUnavailable,
    OutcomeUncertain,
}

impl std::fmt::Display for ManagedSiteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "managed site input is invalid",
            Self::UnsafePath => "managed site path is unsafe",
            Self::NotManaged => "site config is not owned by FusionPanel",
            Self::Io => "managed site filesystem operation failed",
            Self::CandidateRejected => "Nginx rejected the staged site config",
            Self::ActiveConfigRejected => "Nginx rejected the active config",
            Self::ReloadFailed => "Nginx reload failed",
            Self::RollbackFailed => "site config rollback could not be completed",
            Self::HelperUnavailable => "privileged site helper is unavailable",
            Self::OutcomeUncertain => "site operation outcome is uncertain",
        })
    }
}

impl std::error::Error for ManagedSiteError {}

pub trait ManagedSiteOperations: Send + Sync {
    fn apply_site(
        &self,
        site_id: Uuid,
        domain: &str,
        spec: &ManagedSiteSpec,
    ) -> Result<(), ManagedSiteError>;

    fn delete_site(&self, site_id: Uuid, domain: &str) -> Result<(), ManagedSiteError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedEntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

/// Filesystem boundary for unit tests and for rejecting symlinked managed paths.
pub trait ManagedSiteFs: Send + Sync {
    fn entry_kind(&self, path: &Path) -> Result<Option<ManagedEntryKind>, ManagedSiteError>;
    fn ensure_directory(&self, path: &Path) -> Result<(), ManagedSiteError>;
    fn read_link(&self, path: &Path) -> Result<PathBuf, ManagedSiteError>;
    fn read_file(&self, path: &Path) -> Result<Vec<u8>, ManagedSiteError>;
    fn write_new_file(&self, path: &Path, content: &[u8]) -> Result<(), ManagedSiteError>;
    fn create_symlink(&self, target: &Path, link: &Path) -> Result<(), ManagedSiteError>;
    fn rename(&self, from: &Path, to: &Path) -> Result<(), ManagedSiteError>;
    fn remove_file(&self, path: &Path) -> Result<(), ManagedSiteError>;
    fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, ManagedSiteError>;
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, ManagedSiteError>;
}

#[derive(Default)]
pub struct StdManagedSiteFs;

impl ManagedSiteFs for StdManagedSiteFs {
    fn entry_kind(&self, path: &Path) -> Result<Option<ManagedEntryKind>, ManagedSiteError> {
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                let file_type = metadata.file_type();
                Ok(Some(if file_type.is_symlink() {
                    ManagedEntryKind::Symlink
                } else if file_type.is_dir() {
                    ManagedEntryKind::Directory
                } else if file_type.is_file() {
                    ManagedEntryKind::File
                } else {
                    ManagedEntryKind::Other
                }))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(ManagedSiteError::Io),
        }
    }

    fn ensure_directory(&self, path: &Path) -> Result<(), ManagedSiteError> {
        match self.entry_kind(path)? {
            Some(ManagedEntryKind::Directory) => Ok(()),
            Some(_) => Err(ManagedSiteError::UnsafePath),
            None => {
                let mut builder = fs::DirBuilder::new();
                builder.recursive(false);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o750);
                }
                match builder.create(path) {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        if self.entry_kind(path)? == Some(ManagedEntryKind::Directory) {
                            Ok(())
                        } else {
                            Err(ManagedSiteError::UnsafePath)
                        }
                    }
                    Err(_) => Err(ManagedSiteError::Io),
                }
            }
        }
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, ManagedSiteError> {
        fs::read_link(path).map_err(|_| ManagedSiteError::Io)
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, ManagedSiteError> {
        fs::read(path).map_err(|_| ManagedSiteError::Io)
    }

    fn write_new_file(&self, path: &Path, content: &[u8]) -> Result<(), ManagedSiteError> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o640);
        }
        let mut file = options.open(path).map_err(|_| ManagedSiteError::Io)?;
        if file.write_all(content).is_err() || file.sync_all().is_err() {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(ManagedSiteError::Io);
        }
        Ok(())
    }

    fn create_symlink(&self, target: &Path, link: &Path) -> Result<(), ManagedSiteError> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).map_err(|_| ManagedSiteError::Io)
        }
        #[cfg(not(unix))]
        {
            let _ = (target, link);
            Err(ManagedSiteError::Io)
        }
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), ManagedSiteError> {
        fs::rename(from, to).map_err(|_| ManagedSiteError::Io)
    }

    fn remove_file(&self, path: &Path) -> Result<(), ManagedSiteError> {
        fs::remove_file(path).map_err(|_| ManagedSiteError::Io)
    }

    fn list_directory(&self, path: &Path) -> Result<Vec<PathBuf>, ManagedSiteError> {
        fs::read_dir(path)
            .map_err(|_| ManagedSiteError::Io)?
            .map(|entry| {
                entry
                    .map(|entry| entry.path())
                    .map_err(|_| ManagedSiteError::Io)
            })
            .collect()
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, ManagedSiteError> {
        fs::canonicalize(path).map_err(|_| ManagedSiteError::Io)
    }
}

pub trait NginxControl: Send + Sync {
    fn validate_candidate(&self, config_path: &Path) -> Result<(), ManagedSiteError>;
    fn validate_active(&self, required_include: &str) -> Result<(), ManagedSiteError>;
    fn reload(&self) -> Result<(), ManagedSiteError>;
}

pub struct SystemNginxControl {
    nginx_binary: PathBuf,
    active_config: PathBuf,
    prefix: Option<PathBuf>,
}

impl Default for SystemNginxControl {
    fn default() -> Self {
        Self {
            nginx_binary: PathBuf::from("nginx"),
            active_config: PathBuf::from("/etc/nginx/nginx.conf"),
            prefix: None,
        }
    }
}

impl SystemNginxControl {
    /// Configure the real Nginx runtime used by a node or a colocated fixture.
    /// The config and prefix can point at an isolated Compose mount; no service
    /// manager or shell shim is required.
    pub fn with_runtime(
        nginx_binary: PathBuf,
        active_config: PathBuf,
        prefix: Option<PathBuf>,
    ) -> Self {
        Self {
            nginx_binary,
            active_config,
            prefix,
        }
    }

    fn command_with_prefix(&self) -> Command {
        let mut command = Command::new(&self.nginx_binary);
        if let Some(prefix) = &self.prefix {
            command.arg("-p").arg(prefix);
        }
        command
    }
}

impl NginxControl for SystemNginxControl {
    fn validate_candidate(&self, config_path: &Path) -> Result<(), ManagedSiteError> {
        let status = self
            .command_with_prefix()
            .args(["-t", "-q", "-c"])
            .arg(config_path)
            .output()
            .map_err(|_| ManagedSiteError::CandidateRejected)?;
        if status.status.success() {
            Ok(())
        } else {
            eprintln!(
                "nginx validate_candidate failed (status={}): {}",
                status.status,
                String::from_utf8_lossy(&status.stderr).trim()
            );
            Err(ManagedSiteError::CandidateRejected)
        }
    }

    fn validate_active(&self, required_include: &str) -> Result<(), ManagedSiteError> {
        let output = self
            .command_with_prefix()
            .args(["-T", "-c"])
            .arg(&self.active_config)
            .output()
            .map_err(|_| ManagedSiteError::ActiveConfigRejected)?;
        if !output.status.success() {
            return Err(ManagedSiteError::ActiveConfigRejected);
        }
        let printed = String::from_utf8_lossy(&output.stdout);
        if printed.lines().any(|line| line.trim() == required_include) {
            Ok(())
        } else {
            Err(ManagedSiteError::ActiveConfigRejected)
        }
    }

    fn reload(&self) -> Result<(), ManagedSiteError> {
        let status = self
            .command_with_prefix()
            .args(["-s", "reload", "-c"])
            .arg(&self.active_config)
            .output()
            .map_err(|_| ManagedSiteError::ReloadFailed)?;
        if status.status.success() {
            Ok(())
        } else {
            Err(ManagedSiteError::ReloadFailed)
        }
    }
}

pub struct NginxSiteAdapter {
    managed_root: PathBuf,
    document_root_base: PathBuf,
    filesystem: Arc<dyn ManagedSiteFs>,
    nginx: Arc<dyn NginxControl>,
    transaction_lock: Mutex<()>,
}

impl Default for NginxSiteAdapter {
    fn default() -> Self {
        Self::system_default()
    }
}

impl NginxSiteAdapter {
    /// Use node defaults, optionally overridden by the `FUSIONPANEL_NGINX_*`
    /// and `FUSIONPANEL_SITE_DOCUMENT_ROOT_BASE` environment variables. This
    /// makes the real CLI boundary configurable in isolated agent/Nginx fixtures.
    pub fn system_default() -> Self {
        let managed_root = env_path_or("FUSIONPANEL_NGINX_MANAGED_ROOT", DEFAULT_MANAGED_ROOT);
        let document_root_base =
            env_path_or("FUSIONPANEL_SITE_DOCUMENT_ROOT_BASE", DEFAULT_DOCUMENT_ROOT);
        let nginx_binary = env_path_or("FUSIONPANEL_NGINX_BINARY", "nginx");
        let active_config = env_path_or("FUSIONPANEL_NGINX_CONFIG", "/etc/nginx/nginx.conf");
        let prefix = std::env::var_os("FUSIONPANEL_NGINX_PREFIX").map(PathBuf::from);
        Self::with_runtime(
            managed_root,
            document_root_base,
            nginx_binary,
            active_config,
            prefix,
        )
    }

    /// Construct an adapter against explicit filesystem roots and a real Nginx
    /// CLI runtime. This supports isolated container fixtures without replacing
    /// Nginx commands with a service-manager shim.
    pub fn with_runtime(
        managed_root: PathBuf,
        document_root_base: PathBuf,
        nginx_binary: PathBuf,
        active_config: PathBuf,
        prefix: Option<PathBuf>,
    ) -> Self {
        Self::with_components(
            managed_root,
            document_root_base,
            Arc::new(StdManagedSiteFs),
            Arc::new(SystemNginxControl::with_runtime(
                nginx_binary,
                active_config,
                prefix,
            )),
        )
    }

    pub fn with_components(
        managed_root: PathBuf,
        document_root_base: PathBuf,
        filesystem: Arc<dyn ManagedSiteFs>,
        nginx: Arc<dyn NginxControl>,
    ) -> Self {
        Self {
            managed_root,
            document_root_base,
            filesystem,
            nginx,
            transaction_lock: Mutex::new(()),
        }
    }

    fn validate_root_path(&self) -> Result<(), ManagedSiteError> {
        validate_clean_absolute_path(&self.managed_root)?;
        validate_clean_absolute_path(&self.document_root_base)
    }

    fn prepare_layout(&self) -> Result<Layout, ManagedSiteError> {
        self.validate_root_path()?;
        ensure_managed_root(self.filesystem.as_ref(), &self.managed_root)?;
        let available = self.managed_root.join("sites-available");
        let enabled = self.managed_root.join("sites-enabled");
        let staging = self.managed_root.join("staging");
        for path in [&available, &enabled, &staging] {
            self.filesystem.ensure_directory(path)?;
        }
        Ok(Layout {
            available,
            enabled,
            staging,
        })
    }

    fn validate_document_root(
        &self,
        domain: &str,
        document_root: &str,
    ) -> Result<(), ManagedSiteError> {
        if !valid_domain(domain)
            || !valid_document_root(domain, document_root, &self.document_root_base)
        {
            return Err(ManagedSiteError::InvalidInput);
        }
        let path = Path::new(document_root);
        reject_symlink_components(self.filesystem.as_ref(), path)?;
        if self.filesystem.entry_kind(path)? != Some(ManagedEntryKind::Directory) {
            return Err(ManagedSiteError::InvalidInput);
        }
        let canonical_base = self.filesystem.canonicalize(&self.document_root_base)?;
        let canonical_path = self.filesystem.canonicalize(path)?;
        if !canonical_path.starts_with(canonical_base.join(domain)) {
            return Err(ManagedSiteError::UnsafePath);
        }
        Ok(())
    }

    fn current_target(
        &self,
        layout: &Layout,
        site_id: Uuid,
        domain: &str,
    ) -> Result<Option<PathBuf>, ManagedSiteError> {
        let active = layout.enabled.join(format!("{domain}.conf"));
        match self.filesystem.entry_kind(&active)? {
            None => Ok(None),
            Some(ManagedEntryKind::Symlink) => {
                let target = self.filesystem.read_link(&active)?;
                let resolved = target.clone();
                validate_managed_target(self.filesystem.as_ref(), &layout.available, &resolved)?;
                if !resolved.starts_with(&layout.available)
                    || self.filesystem.entry_kind(&resolved)? != Some(ManagedEntryKind::File)
                {
                    return Err(ManagedSiteError::UnsafePath);
                }
                let content = self.filesystem.read_file(&resolved)?;
                if !managed_marker_matches(&content, site_id, domain) {
                    return Err(ManagedSiteError::NotManaged);
                }
                Ok(Some(target))
            }
            Some(_) => Err(ManagedSiteError::UnsafePath),
        }
    }

    fn list_enabled_configs(
        &self,
        layout: &Layout,
        exclude_domain: Option<&str>,
    ) -> Result<Vec<PathBuf>, ManagedSiteError> {
        let mut paths = Vec::new();
        for path in self.filesystem.list_directory(&layout.enabled)? {
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(ManagedSiteError::UnsafePath)?;
            if !file_name.ends_with(".conf") {
                continue;
            }
            let domain = file_name
                .strip_suffix(".conf")
                .ok_or(ManagedSiteError::UnsafePath)?;
            if !valid_domain(domain) {
                return Err(ManagedSiteError::UnsafePath);
            }
            if exclude_domain == Some(domain) {
                continue;
            }
            if self.filesystem.entry_kind(&path)? != Some(ManagedEntryKind::Symlink) {
                return Err(ManagedSiteError::UnsafePath);
            }
            let target = self.filesystem.read_link(&path)?;
            let resolved = target;
            validate_managed_target(self.filesystem.as_ref(), &layout.available, &resolved)?;
            if !resolved.starts_with(&layout.available)
                || self.filesystem.entry_kind(&resolved)? != Some(ManagedEntryKind::File)
            {
                return Err(ManagedSiteError::UnsafePath);
            }
            paths.push(resolved);
        }
        paths.sort();
        Ok(paths)
    }

    fn write_candidate_test_config(
        &self,
        layout: &Layout,
        candidate: Option<&Path>,
        ignored_domain: Option<&str>,
        nonce: Uuid,
    ) -> Result<PathBuf, ManagedSiteError> {
        let mut includes = self.list_enabled_configs(layout, ignored_domain)?;
        if let Some(candidate) = candidate {
            includes.push(candidate.to_path_buf());
        }
        let config = test_main_config(&includes)?;
        let test_path = layout.staging.join(format!("test-{nonce}.conf"));
        self.filesystem
            .write_new_file(&test_path, config.as_bytes())?;
        Ok(test_path)
    }

    fn atomic_replace_link(
        &self,
        layout: &Layout,
        domain: &str,
        target: &Path,
        nonce: Uuid,
    ) -> Result<(), ManagedSiteError> {
        let active = layout.enabled.join(format!("{domain}.conf"));
        let temporary = layout.enabled.join(format!(".link-{nonce}"));
        if self.filesystem.entry_kind(&temporary)?.is_some() {
            return Err(ManagedSiteError::UnsafePath);
        }
        if !matches!(
            self.filesystem.entry_kind(&active)?,
            None | Some(ManagedEntryKind::Symlink)
        ) {
            return Err(ManagedSiteError::UnsafePath);
        }
        self.filesystem.create_symlink(target, &temporary)?;
        if let Err(error) = self.filesystem.rename(&temporary, &active) {
            let _ = self.filesystem.remove_file(&temporary);
            return Err(error);
        }
        Ok(())
    }

    fn restore_link(
        &self,
        layout: &Layout,
        domain: &str,
        prior_target: Option<&Path>,
        nonce: Uuid,
    ) -> Result<(), ManagedSiteError> {
        let active = layout.enabled.join(format!("{domain}.conf"));
        match prior_target {
            Some(target) => self.atomic_replace_link(layout, domain, target, nonce),
            None => match self.filesystem.entry_kind(&active)? {
                None => Ok(()),
                Some(ManagedEntryKind::Symlink) => self.filesystem.remove_file(&active),
                Some(_) => Err(ManagedSiteError::UnsafePath),
            },
        }
    }

    fn remove_if_file(&self, path: &Path) {
        if self.filesystem.entry_kind(path).ok().flatten() == Some(ManagedEntryKind::File) {
            let _ = self.filesystem.remove_file(path);
        }
    }

    fn rollback_after_reload_failure(
        &self,
        layout: &Layout,
        domain: &str,
        prior_target: Option<&Path>,
        nonce: Uuid,
    ) -> Result<(), ManagedSiteError> {
        self.restore_link(layout, domain, prior_target, nonce)
            .map_err(|_| ManagedSiteError::RollbackFailed)?;
        let required_include = self.required_include();
        self.nginx
            .validate_active(&required_include)
            .map_err(|_| ManagedSiteError::RollbackFailed)?;
        self.nginx
            .reload()
            .map_err(|_| ManagedSiteError::RollbackFailed)
    }

    fn required_include(&self) -> String {
        format!(
            "include {}/sites-enabled/*.conf;",
            self.managed_root.display()
        )
    }
}

fn env_path_or(name: &str, default: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

impl ManagedSiteOperations for NginxSiteAdapter {
    fn apply_site(
        &self,
        site_id: Uuid,
        domain: &str,
        spec: &ManagedSiteSpec,
    ) -> Result<(), ManagedSiteError> {
        let _transaction = self
            .transaction_lock
            .lock()
            .map_err(|_| ManagedSiteError::Io)?;
        if site_id.is_nil() || !valid_domain(domain) {
            return Err(ManagedSiteError::InvalidInput);
        }
        let config = match spec {
            ManagedSiteSpec::Static { document_root } => {
                self.validate_document_root(domain, document_root)?;
                render_static_site(site_id, domain, document_root)
            }
            ManagedSiteSpec::ReverseProxy { upstream_url } => {
                if !valid_upstream_url(upstream_url) {
                    return Err(ManagedSiteError::InvalidInput);
                }
                render_reverse_proxy_site(site_id, domain, upstream_url)
            }
        };
        let layout = self.prepare_layout()?;
        let prior_target = self.current_target(&layout, site_id, domain)?;
        let nonce = Uuid::new_v4();
        let candidate_path = layout.available.join(format!("{site_id}-{nonce}.conf"));

        self.filesystem
            .write_new_file(&candidate_path, config.as_bytes())?;
        let test_path = match self.write_candidate_test_config(
            &layout,
            Some(&candidate_path),
            Some(domain),
            nonce,
        ) {
            Ok(path) => path,
            Err(error) => {
                self.remove_if_file(&candidate_path);
                return Err(error);
            }
        };
        if self.nginx.validate_candidate(&test_path).is_err() {
            self.remove_if_file(&test_path);
            self.remove_if_file(&candidate_path);
            return Err(ManagedSiteError::CandidateRejected);
        }

        let active = layout.enabled.join(format!("{domain}.conf"));
        if let Err(error) = self.atomic_replace_link(&layout, domain, &candidate_path, nonce) {
            self.remove_if_file(&test_path);
            self.remove_if_file(&candidate_path);
            return Err(error);
        }

        let required_include = self.required_include();
        if self.nginx.validate_active(&required_include).is_err() {
            let rollback =
                self.restore_link(&layout, domain, prior_target.as_deref(), Uuid::new_v4());
            self.remove_if_file(&test_path);
            self.remove_if_file(&candidate_path);
            return match rollback {
                Ok(()) => Err(ManagedSiteError::ActiveConfigRejected),
                Err(_) => Err(ManagedSiteError::RollbackFailed),
            };
        }
        if self.nginx.reload().is_err() {
            let rollback = self.rollback_after_reload_failure(
                &layout,
                domain,
                prior_target.as_deref(),
                Uuid::new_v4(),
            );
            self.remove_if_file(&test_path);
            self.remove_if_file(&candidate_path);
            return rollback.and(Err(ManagedSiteError::ReloadFailed));
        }

        self.remove_if_file(&test_path);
        if let Some(old_target) = prior_target {
            let old_path = if old_target.is_absolute() {
                old_target
            } else {
                active
                    .parent()
                    .ok_or(ManagedSiteError::UnsafePath)?
                    .join(old_target)
            };
            if old_path != candidate_path {
                self.remove_if_file(&old_path);
            }
        }
        Ok(())
    }

    fn delete_site(&self, site_id: Uuid, domain: &str) -> Result<(), ManagedSiteError> {
        let _transaction = self
            .transaction_lock
            .lock()
            .map_err(|_| ManagedSiteError::Io)?;
        if site_id.is_nil() || !valid_domain(domain) {
            return Err(ManagedSiteError::InvalidInput);
        }
        let layout = self.prepare_layout()?;
        let Some(prior_target) = self.current_target(&layout, site_id, domain)? else {
            return Ok(());
        };
        let nonce = Uuid::new_v4();
        let tombstone = layout.staging.join(format!("deleted-{nonce}"));
        let candidate_test = self.write_candidate_test_config(&layout, None, Some(domain), nonce);
        let test_path = candidate_test?;
        if self.nginx.validate_candidate(&test_path).is_err() {
            self.remove_if_file(&test_path);
            return Err(ManagedSiteError::CandidateRejected);
        }

        let active = layout.enabled.join(format!("{domain}.conf"));
        if let Err(error) = self.filesystem.rename(&active, &tombstone) {
            self.remove_if_file(&test_path);
            return Err(error);
        }
        let required_include = self.required_include();
        if self.nginx.validate_active(&required_include).is_err() {
            let rollback = self
                .filesystem
                .rename(&tombstone, &active)
                .map_err(|_| ManagedSiteError::RollbackFailed);
            self.remove_if_file(&test_path);
            return match rollback {
                Ok(()) => Err(ManagedSiteError::ActiveConfigRejected),
                Err(error) => Err(error),
            };
        }
        if self.nginx.reload().is_err() {
            let restore = self.filesystem.rename(&tombstone, &active);
            let recovery = if restore.is_ok() {
                self.nginx
                    .validate_active(&required_include)
                    .and_then(|()| self.nginx.reload())
            } else {
                Err(ManagedSiteError::RollbackFailed)
            };
            self.remove_if_file(&test_path);
            return recovery
                .map_err(|_| ManagedSiteError::RollbackFailed)
                .and(Err(ManagedSiteError::ReloadFailed));
        }

        self.remove_if_file(&test_path);
        let _ = self.filesystem.remove_file(&tombstone);
        let old_path = if prior_target.is_absolute() {
            prior_target
        } else {
            active
                .parent()
                .ok_or(ManagedSiteError::UnsafePath)?
                .join(prior_target)
        };
        self.remove_if_file(&old_path);
        Ok(())
    }
}

struct Layout {
    available: PathBuf,
    enabled: PathBuf,
    staging: PathBuf,
}

fn ensure_managed_root(fs: &dyn ManagedSiteFs, path: &Path) -> Result<(), ManagedSiteError> {
    let components: Vec<_> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part),
            Component::RootDir => None,
            _ => None,
        })
        .collect();
    if components.is_empty() {
        return Err(ManagedSiteError::UnsafePath);
    }
    let mut current = PathBuf::new();
    current.push("/");
    for (index, part) in components.iter().enumerate() {
        current.push(part);
        match fs.entry_kind(&current)? {
            Some(ManagedEntryKind::Directory) => {}
            None if index + 1 == components.len() => fs.ensure_directory(&current)?,
            _ => return Err(ManagedSiteError::UnsafePath),
        }
    }
    if current != path || fs.entry_kind(path)? != Some(ManagedEntryKind::Directory) {
        return Err(ManagedSiteError::UnsafePath);
    }
    Ok(())
}

fn reject_symlink_components(fs: &dyn ManagedSiteFs, path: &Path) -> Result<(), ManagedSiteError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir => current.push("/"),
            Component::Normal(part) => {
                current.push(part);
                match fs.entry_kind(&current)? {
                    Some(ManagedEntryKind::Symlink) => return Err(ManagedSiteError::UnsafePath),
                    Some(ManagedEntryKind::Directory) if current != path => {}
                    Some(ManagedEntryKind::Directory) => {}
                    Some(ManagedEntryKind::Other) => return Err(ManagedSiteError::UnsafePath),
                    Some(ManagedEntryKind::File) if current == path => {}
                    Some(ManagedEntryKind::File) => return Err(ManagedSiteError::UnsafePath),
                    None => return Err(ManagedSiteError::InvalidInput),
                }
            }
            _ => return Err(ManagedSiteError::UnsafePath),
        }
    }
    Ok(())
}

fn validate_managed_target(
    fs: &dyn ManagedSiteFs,
    available: &Path,
    target: &Path,
) -> Result<(), ManagedSiteError> {
    validate_clean_absolute_path(target)?;
    if !target.starts_with(available) {
        return Err(ManagedSiteError::UnsafePath);
    }
    let canonical_available = fs.canonicalize(available)?;
    let canonical_target = fs.canonicalize(target)?;
    if !canonical_target.starts_with(canonical_available) {
        return Err(ManagedSiteError::UnsafePath);
    }
    reject_symlink_components(fs, target)
}

fn validate_clean_absolute_path(path: &Path) -> Result<(), ManagedSiteError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
        || !path
            .to_string_lossy()
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        return Err(ManagedSiteError::UnsafePath);
    }
    Ok(())
}

fn valid_domain(domain: &str) -> bool {
    !domain.is_empty()
        && domain.len() <= 253
        && domain == domain.to_ascii_lowercase()
        && domain.parse::<std::net::IpAddr>().is_err()
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn valid_document_root(domain: &str, raw: &str, base: &Path) -> bool {
    if raw.len() > 4096 || !raw.starts_with('/') || raw.contains('\\') || raw.contains('\0') {
        return false;
    }
    let path = Path::new(raw);
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return false;
    }
    let site_root = base.join(domain);
    path.starts_with(&site_root)
        && path
            .to_string_lossy()
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
}

fn valid_upstream_url(raw: &str) -> bool {
    if raw.is_empty()
        || raw.len() > 2048
        || !raw.is_ascii()
        || raw.chars().any(|ch| {
            ch.is_whitespace()
                || ch.is_control()
                || matches!(
                    ch,
                    '\\' | '"' | '\'' | ';' | '{' | '}' | '$' | '`' | '?' | '#'
                )
        })
    {
        return false;
    }
    let Some((scheme, remainder)) = raw.split_once("://") else {
        return false;
    };
    if !matches!(scheme, "http" | "https") {
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
        (host, port)
    } else {
        if authority.contains('[') || authority.contains(']') {
            return false;
        }
        match authority.rsplit_once(':') {
            Some((host, port)) if !host.contains(':') => (host, Some(port)),
            Some(_) => return false,
            None => (authority, None),
        }
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
    let path = &remainder[authority.len()..];
    if !path.is_empty() && !path.starts_with('/') {
        return false;
    }
    let mut bytes = path.bytes().peekable();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let Some(first) = bytes.next() else {
                return false;
            };
            let Some(second) = bytes.next() else {
                return false;
            };
            if !first.is_ascii_hexdigit() || !second.is_ascii_hexdigit() {
                return false;
            }
        } else if !(byte.is_ascii_alphanumeric()
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
    }
    !Path::new(path)
        .components()
        .any(|component| matches!(component, Component::ParentDir))
}

fn render_static_site(site_id: Uuid, domain: &str, document_root: &str) -> String {
    format!(
        "# FusionPanel managed site: id={site_id} domain={domain}\nserver {{\n    listen 80;\n    server_name {domain};\n    root {document_root};\n    location / {{\n        try_files $uri $uri/ =404;\n    }}\n}}\n"
    )
}

fn render_reverse_proxy_site(site_id: Uuid, domain: &str, upstream_url: &str) -> String {
    format!(
        "# FusionPanel managed site: id={site_id} domain={domain}\nserver {{\n    listen 80;\n    server_name {domain};\n    location / {{\n        proxy_pass {upstream_url};\n        proxy_set_header Host $host;\n        proxy_set_header X-Real-IP $remote_addr;\n        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n        proxy_set_header X-Forwarded-Proto $scheme;\n    }}\n}}\n"
    )
}

fn test_main_config(includes: &[PathBuf]) -> Result<String, ManagedSiteError> {
    let mut config = String::from(
        "user root;\nworker_processes 1;\npid /tmp/fusionpanel-nginx-test.pid;\nlock_file /tmp/fusionpanel-nginx-test.lock;\nerror_log stderr emerg;\nevents { worker_connections 64; }\nhttp {\n    access_log off;\n    client_body_temp_path /tmp/fusionpanel-nginx-test-body;\n    proxy_temp_path /tmp/fusionpanel-nginx-test-proxy;\n    fastcgi_temp_path /tmp/fusionpanel-nginx-test-fastcgi;\n    uwsgi_temp_path /tmp/fusionpanel-nginx-test-uwsgi;\n    scgi_temp_path /tmp/fusionpanel-nginx-test-scgi;\n",
    );
    for include in includes {
        validate_clean_absolute_path(include)?;
        config.push_str(&format!("    include {};\n", include.display()));
    }
    config.push_str("}\n");
    Ok(config)
}

fn managed_marker_matches(content: &[u8], site_id: Uuid, domain: &str) -> bool {
    let marker = format!("# FusionPanel managed site: id={site_id} domain={domain}");
    content
        .split(|byte| *byte == b'\n')
        .next()
        .is_some_and(|first_line| first_line == marker.as_bytes())
}
