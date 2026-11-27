#![forbid(unsafe_code)]

use fusionpanel_agent_protocol::{
    Operation, Request, Response, ResponseError, ResponseResult, SiteApplyPayload,
    SiteDeletePayload, SiteSpec, TlsApplyPayload, TlsCertificateSource, TlsDeletePayload,
};
use hosting::{ManagedSiteError, ManagedSiteOperations, ManagedSiteSpec, NginxSiteAdapter};
use serde_json::json;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

pub const HELPER_SOCKET_PATH: &str = "/run/fusionpanel-nginx/nginx-helper.sock";
const MAX_RPC_REQUEST_BYTES: usize = 128 * 1024;
const MAX_RPC_RESPONSE_BYTES: usize = 4 * 1024;
const RPC_IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct NginxHelperClient {
    socket_path: PathBuf,
}

impl Default for NginxHelperClient {
    fn default() -> Self {
        let socket_path = std::env::var_os("FUSIONPANEL_NGINX_HELPER_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(HELPER_SOCKET_PATH));
        Self::new(socket_path)
    }
}

impl NginxHelperClient {
    /// The production caller uses the fixed helper socket. This constructor is
    /// exposed so unit tests can use a socket inside their temporary directory.
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    fn call(&self, operation: Operation) -> Result<(), ManagedSiteError> {
        let request = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation,
        };
        request
            .validate()
            .map_err(|_| ManagedSiteError::InvalidInput)?;
        let request_id = request.request_id;
        let bytes = serde_json::to_vec(&request).map_err(|_| ManagedSiteError::Io)?;
        if bytes.len() > MAX_RPC_REQUEST_BYTES {
            return Err(ManagedSiteError::InvalidInput);
        }

        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|_| ManagedSiteError::HelperUnavailable)?;
        stream
            .set_read_timeout(Some(RPC_IO_TIMEOUT))
            .map_err(|_| ManagedSiteError::HelperUnavailable)?;
        stream
            .set_write_timeout(Some(RPC_IO_TIMEOUT))
            .map_err(|_| ManagedSiteError::HelperUnavailable)?;
        stream
            .write_all(&bytes)
            .map_err(|_| ManagedSiteError::OutcomeUncertain)?;
        stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(|_| ManagedSiteError::OutcomeUncertain)?;

        let response_bytes = read_bounded(&mut stream, MAX_RPC_RESPONSE_BYTES)
            .map_err(|_| ManagedSiteError::OutcomeUncertain)?;
        let response: Response = serde_json::from_slice(&response_bytes)
            .map_err(|_| ManagedSiteError::OutcomeUncertain)?;
        if response.version != 1 || response.request_id != request_id {
            return Err(ManagedSiteError::OutcomeUncertain);
        }
        match response.result {
            ResponseResult::Result(value) if value == json!({ "status": "applied" }) => Ok(()),
            ResponseResult::Result(_) => Err(ManagedSiteError::OutcomeUncertain),
            ResponseResult::Error(error) => Err(error_to_managed_site_error(&error)),
        }
    }
}

impl ManagedSiteOperations for NginxHelperClient {
    fn apply_site(
        &self,
        site_id: Uuid,
        domain: &str,
        spec: &ManagedSiteSpec,
    ) -> Result<(), ManagedSiteError> {
        let spec = match spec {
            ManagedSiteSpec::Static { document_root } => SiteSpec::Static {
                document_root: document_root.clone(),
            },
            ManagedSiteSpec::ReverseProxy { upstream_url } => SiteSpec::ReverseProxy {
                upstream_url: upstream_url.clone(),
            },
        };
        self.call(Operation::SiteApply(SiteApplyPayload {
            site_id,
            domain: domain.to_string(),
            spec,
        }))
    }

    fn delete_site(&self, site_id: Uuid, domain: &str) -> Result<(), ManagedSiteError> {
        self.call(Operation::SiteDelete(SiteDeletePayload {
            site_id,
            domain: domain.to_string(),
        }))
    }

    fn apply_site_tls(
        &self,
        site_id: Uuid,
        domain: &str,
        spec: &ManagedSiteSpec,
        cert_pem: &str,
        key_pem: &str,
    ) -> Result<(), ManagedSiteError> {
        let spec = match spec {
            ManagedSiteSpec::Static { document_root } => SiteSpec::Static {
                document_root: document_root.clone(),
            },
            ManagedSiteSpec::ReverseProxy { upstream_url } => SiteSpec::ReverseProxy {
                upstream_url: upstream_url.clone(),
            },
        };
        self.call(Operation::TlsApply(TlsApplyPayload {
            site_id,
            domain: domain.to_string(),
            spec,
            source: TlsCertificateSource::CustomPem {
                certificate_pem: cert_pem.to_string(),
                private_key_pem: key_pem.to_string(),
            },
        }))
    }

    fn delete_site_tls(
        &self,
        site_id: Uuid,
        domain: &str,
        spec: &ManagedSiteSpec,
    ) -> Result<(), ManagedSiteError> {
        let spec = match spec {
            ManagedSiteSpec::Static { document_root } => SiteSpec::Static {
                document_root: document_root.clone(),
            },
            ManagedSiteSpec::ReverseProxy { upstream_url } => SiteSpec::ReverseProxy {
                upstream_url: upstream_url.clone(),
            },
        };
        self.call(Operation::TlsDelete(TlsDeletePayload {
            site_id,
            domain: domain.to_string(),
            spec,
        }))
    }
}

/// Starts the standalone privileged service. It does not read agent or API
/// environment files; every filesystem path and the Nginx executable are fixed.
pub fn run_helper_service() -> io::Result<()> {
    run_helper_service_at(Path::new(HELPER_SOCKET_PATH))
}

/// Starts the helper at an explicit local socket path for isolated fixtures.
/// The installed service uses `run_helper_service()` with its fixed path.
pub fn run_helper_service_at(socket_path: &Path) -> io::Result<()> {
    let adapter = NginxSiteAdapter::with_runtime(
        PathBuf::from("/etc/nginx/fusionpanel"),
        PathBuf::from("/srv/fusionpanel/sites"),
        PathBuf::from("/usr/sbin/nginx"),
        PathBuf::from("/etc/nginx/nginx.conf"),
        None,
    );
    let operations = RootOwnedManagedSiteOperations { adapter };
    let listener = bind_socket(socket_path)?;
    loop {
        match listener.accept() {
            Ok((stream, _)) => serve_connection(stream, &operations),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

struct RootOwnedManagedSiteOperations {
    adapter: NginxSiteAdapter,
}

impl ManagedSiteOperations for RootOwnedManagedSiteOperations {
    fn apply_site(
        &self,
        site_id: Uuid,
        domain: &str,
        spec: &ManagedSiteSpec,
    ) -> Result<(), ManagedSiteError> {
        validate_managed_config_tree(Path::new("/etc/nginx/fusionpanel"), 0)?;
        self.adapter.apply_site(site_id, domain, spec)
    }

    fn delete_site(&self, site_id: Uuid, domain: &str) -> Result<(), ManagedSiteError> {
        validate_managed_config_tree(Path::new("/etc/nginx/fusionpanel"), 0)?;
        self.adapter.delete_site(site_id, domain)
    }

    fn apply_site_tls(
        &self,
        site_id: Uuid,
        domain: &str,
        spec: &ManagedSiteSpec,
        cert_pem: &str,
        key_pem: &str,
    ) -> Result<(), ManagedSiteError> {
        validate_managed_config_tree(Path::new("/etc/nginx/fusionpanel"), 0)?;
        self.adapter
            .apply_site_tls(site_id, domain, spec, cert_pem, key_pem)
    }

    fn delete_site_tls(
        &self,
        site_id: Uuid,
        domain: &str,
        spec: &ManagedSiteSpec,
    ) -> Result<(), ManagedSiteError> {
        validate_managed_config_tree(Path::new("/etc/nginx/fusionpanel"), 0)?;
        self.adapter.delete_site_tls(site_id, domain, spec)
    }
}

fn validate_managed_config_tree(
    managed_root: &Path,
    expected_uid: u32,
) -> Result<(), ManagedSiteError> {
    let nginx_root = managed_root.parent().ok_or(ManagedSiteError::UnsafePath)?;
    let etc_root = nginx_root.parent().ok_or(ManagedSiteError::UnsafePath)?;
    let available = managed_root.join("sites-available");
    let enabled = managed_root.join("sites-enabled");
    let staging = managed_root.join("staging");

    for (path, missing_allowed) in [
        (etc_root, false),
        (nginx_root, false),
        (managed_root, true),
        (available.as_path(), true),
        (enabled.as_path(), true),
        (staging.as_path(), true),
    ] {
        validate_owned_directory(path, expected_uid, missing_allowed)?;
    }

    validate_managed_directory_entries(&available, expected_uid, false)?;
    validate_managed_directory_entries(&staging, expected_uid, false)?;
    validate_managed_directory_entries(&enabled, expected_uid, true)?;
    Ok(())
}

fn validate_owned_directory(
    path: &Path,
    expected_uid: u32,
    missing_allowed: bool,
) -> Result<bool, ManagedSiteError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound && missing_allowed => {
            return Ok(false)
        }
        Err(_) => return Err(ManagedSiteError::UnsafePath),
    };
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != expected_uid
        || metadata.permissions().mode() & 0o022 != 0
    {
        return Err(ManagedSiteError::UnsafePath);
    }
    Ok(true)
}

fn validate_managed_directory_entries(
    directory: &Path,
    expected_uid: u32,
    allow_enabled_links: bool,
) -> Result<(), ManagedSiteError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(ManagedSiteError::UnsafePath),
    };
    let available = directory
        .parent()
        .ok_or(ManagedSiteError::UnsafePath)?
        .join("sites-available");
    for entry in entries {
        let path = entry.map_err(|_| ManagedSiteError::UnsafePath)?.path();
        let metadata = fs::symlink_metadata(&path).map_err(|_| ManagedSiteError::UnsafePath)?;
        let kind = metadata.file_type();
        if kind.is_symlink() {
            if !allow_enabled_links {
                return Err(ManagedSiteError::UnsafePath);
            }
            let target = fs::read_link(&path).map_err(|_| ManagedSiteError::UnsafePath)?;
            if !target.is_absolute()
                || !target.starts_with(&available)
                || target
                    .components()
                    .any(|component| matches!(component, std::path::Component::ParentDir))
            {
                return Err(ManagedSiteError::UnsafePath);
            }
            let target_metadata =
                fs::symlink_metadata(&target).map_err(|_| ManagedSiteError::UnsafePath)?;
            if !target_metadata.is_file()
                || target_metadata.file_type().is_symlink()
                || target_metadata.uid() != expected_uid
                || target_metadata.permissions().mode() & 0o022 != 0
            {
                return Err(ManagedSiteError::UnsafePath);
            }
        } else if !kind.is_file()
            || metadata.uid() != expected_uid
            || metadata.permissions().mode() & 0o022 != 0
        {
            return Err(ManagedSiteError::UnsafePath);
        }
    }
    Ok(())
}

fn bind_socket(path: &Path) -> io::Result<UnixListener> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "helper socket has no parent directory",
        )
    })?;
    let parent_metadata = fs::symlink_metadata(parent)?;
    if !parent_metadata.is_dir() || parent_metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "helper socket directory is not a real directory",
        ));
    }
    match fs::symlink_metadata(path) {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "helper socket path already exists",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let listener = UnixListener::bind(path)?;
    if let Err(error) = fs::set_permissions(path, fs::Permissions::from_mode(0o660)) {
        drop(listener);
        let _ = fs::remove_file(path);
        return Err(error);
    }
    Ok(listener)
}

fn serve_connection(mut stream: UnixStream, operations: &dyn ManagedSiteOperations) {
    let _ = stream.set_read_timeout(Some(RPC_IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(RPC_IO_TIMEOUT));
    let response = match read_bounded(&mut stream, MAX_RPC_REQUEST_BYTES) {
        Ok(bytes) => handle_request(&bytes, operations),
        Err(_) => error_response(Uuid::nil(), "invalid_request", "Invalid helper request"),
    };
    if let Ok(bytes) = serde_json::to_vec(&response) {
        let _ = stream.write_all(&bytes);
    }
}

fn read_bounded(reader: &mut impl Read, max_bytes: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(max_bytes.min(4096));
    let limit = max_bytes.saturating_add(1) as u64;
    reader.take(limit).read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "RPC message exceeded configured bound",
        ));
    }
    Ok(bytes)
}

fn handle_request(bytes: &[u8], operations: &dyn ManagedSiteOperations) -> Response {
    if bytes.len() > MAX_RPC_REQUEST_BYTES {
        return error_response(
            Uuid::nil(),
            "request_too_large",
            "Helper request is too large",
        );
    }
    let request: Request = match serde_json::from_slice(bytes) {
        Ok(request) => request,
        Err(_) => return error_response(Uuid::nil(), "invalid_request", "Invalid helper request"),
    };
    let request_id = request.request_id;
    if request_id.is_nil() || request.validate().is_err() {
        return error_response(request_id, "invalid_request", "Invalid helper request");
    }

    let result = match request.operation {
        Operation::SiteApply(payload) => {
            let spec = match payload.spec {
                SiteSpec::Static { document_root } => ManagedSiteSpec::Static { document_root },
                SiteSpec::ReverseProxy { upstream_url } => {
                    ManagedSiteSpec::ReverseProxy { upstream_url }
                }
            };
            operations.apply_site(payload.site_id, &payload.domain, &spec)
        }
        Operation::SiteDelete(payload) => operations.delete_site(payload.site_id, &payload.domain),
        Operation::TlsApply(payload) => {
            let spec = match payload.spec {
                SiteSpec::Static { document_root } => ManagedSiteSpec::Static { document_root },
                SiteSpec::ReverseProxy { upstream_url } => {
                    ManagedSiteSpec::ReverseProxy { upstream_url }
                }
            };
            match payload.source {
                TlsCertificateSource::CustomPem {
                    certificate_pem,
                    private_key_pem,
                } => operations.apply_site_tls(
                    payload.site_id,
                    &payload.domain,
                    &spec,
                    &certificate_pem,
                    &private_key_pem,
                ),
                TlsCertificateSource::AcmeHttp01 { .. } => {
                    return error_response(
                        request_id,
                        "unsupported_operation",
                        "Privileged helper accepts only resolved PEM certificates",
                    );
                }
            }
        }
        Operation::TlsDelete(payload) => {
            let spec = match payload.spec {
                SiteSpec::Static { document_root } => ManagedSiteSpec::Static { document_root },
                SiteSpec::ReverseProxy { upstream_url } => {
                    ManagedSiteSpec::ReverseProxy { upstream_url }
                }
            };
            operations.delete_site_tls(payload.site_id, &payload.domain, &spec)
        }
        Operation::SystemInfo(_)
        | Operation::SiteValidate(_)
        | Operation::DatabaseApply(_)
        | Operation::DatabaseDelete(_)
        | Operation::DatabaseUserCreate(_)
        | Operation::BackupCreate(_)
        | Operation::BackupRestore(_)
        | Operation::DnsZoneApply(_)
        | Operation::DnsZoneDelete(_)
        | Operation::DnsProbe(_)
        | Operation::CronApply(_)
        | Operation::CronDelete(_)
        | Operation::CronRunNow(_)
        | Operation::MailDomainApply(_)
        | Operation::MailDomainDelete(_)
        | Operation::MailDkimApply(_) => {
            return error_response(
                request_id,
                "unsupported_operation",
                "Only website apply and delete are supported",
            );
        }
    };

    match result {
        Ok(()) => Response {
            version: 1,
            request_id,
            result: ResponseResult::Result(json!({ "status": "applied" })),
        },
        Err(error) => {
            let code = managed_error_code(&error);
            eprintln!("fusionpanel-nginx-helper operation failed: {code}");
            error_response(request_id, code, managed_error_message(&error))
        }
    }
}

fn error_response(request_id: Uuid, code: &str, message: &str) -> Response {
    Response {
        version: 1,
        request_id,
        result: ResponseResult::Error(ResponseError {
            code: code.to_string(),
            message: message.to_string(),
        }),
    }
}

fn managed_error_code(error: &ManagedSiteError) -> &'static str {
    match error {
        ManagedSiteError::InvalidInput => "invalid_input",
        ManagedSiteError::UnsafePath => "unsafe_path",
        ManagedSiteError::NotManaged => "not_managed",
        ManagedSiteError::Io => "io",
        ManagedSiteError::CandidateRejected => "candidate_rejected",
        ManagedSiteError::ActiveConfigRejected => "active_config_rejected",
        ManagedSiteError::ReloadFailed => "reload_failed",
        ManagedSiteError::RollbackFailed => "rollback_failed",
        ManagedSiteError::HelperUnavailable => "helper_unavailable",
        ManagedSiteError::OutcomeUncertain => "operation_uncertain",
    }
}

fn managed_error_message(error: &ManagedSiteError) -> &'static str {
    match error {
        ManagedSiteError::InvalidInput => "Website configuration is invalid",
        ManagedSiteError::UnsafePath => "Website path is unsafe",
        ManagedSiteError::NotManaged => "Website configuration is not managed by FusionPanel",
        ManagedSiteError::Io => "Website configuration could not be read or written",
        ManagedSiteError::CandidateRejected => "Nginx rejected the staged website configuration",
        ManagedSiteError::ActiveConfigRejected => "Active Nginx configuration is incompatible",
        ManagedSiteError::ReloadFailed => {
            "Nginx reload failed; the prior configuration was restored"
        }
        ManagedSiteError::RollbackFailed => {
            "Configuration outcome is uncertain; manual recovery is required"
        }
        ManagedSiteError::HelperUnavailable => "The privileged website helper is unavailable",
        ManagedSiteError::OutcomeUncertain => {
            "Configuration outcome is uncertain; manual recovery is required"
        }
    }
}

fn error_to_managed_site_error(error: &ResponseError) -> ManagedSiteError {
    match error.code.as_str() {
        "invalid_input" => ManagedSiteError::InvalidInput,
        "unsafe_path" => ManagedSiteError::UnsafePath,
        "not_managed" => ManagedSiteError::NotManaged,
        "candidate_rejected" => ManagedSiteError::CandidateRejected,
        "active_config_rejected" => ManagedSiteError::ActiveConfigRejected,
        "reload_failed" => ManagedSiteError::ReloadFailed,
        "rollback_failed" => ManagedSiteError::RollbackFailed,
        "helper_unavailable" => ManagedSiteError::HelperUnavailable,
        "operation_uncertain" => ManagedSiteError::OutcomeUncertain,
        _ => ManagedSiteError::OutcomeUncertain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fusionpanel_agent_protocol::SystemInfoPayload;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::sync::Mutex;
    use tempfile::tempdir;

    #[derive(Default)]
    struct RecordingOperations {
        calls: Mutex<Vec<String>>,
        failure: Mutex<Option<ManagedSiteError>>,
    }

    impl ManagedSiteOperations for RecordingOperations {
        fn apply_site(
            &self,
            _site_id: Uuid,
            domain: &str,
            _spec: &ManagedSiteSpec,
        ) -> Result<(), ManagedSiteError> {
            self.calls.lock().unwrap().push(format!("apply:{domain}"));
            match *self.failure.lock().unwrap() {
                Some(ref error) => Err(error.clone()),
                None => Ok(()),
            }
        }

        fn delete_site(&self, _site_id: Uuid, domain: &str) -> Result<(), ManagedSiteError> {
            self.calls.lock().unwrap().push(format!("delete:{domain}"));
            match *self.failure.lock().unwrap() {
                Some(ref error) => Err(error.clone()),
                None => Ok(()),
            }
        }
    }

    fn apply_request() -> Request {
        Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::SiteApply(SiteApplyPayload {
                site_id: Uuid::new_v4(),
                domain: "example.test".to_string(),
                spec: SiteSpec::Static {
                    document_root: "/srv/fusionpanel/sites/example.test".to_string(),
                },
            }),
        }
    }

    #[test]
    fn helper_accepts_only_typed_site_apply_and_delete() {
        let operations = RecordingOperations::default();
        let response = handle_request(&serde_json::to_vec(&apply_request()).unwrap(), &operations);
        assert!(matches!(response.result, ResponseResult::Result(_)));
        assert_eq!(*operations.calls.lock().unwrap(), ["apply:example.test"]);

        let request = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation: Operation::SystemInfo(SystemInfoPayload {}),
        };
        let response = handle_request(&serde_json::to_vec(&request).unwrap(), &operations);
        assert!(matches!(
            response.result,
            ResponseResult::Error(ResponseError { ref code, .. }) if code == "unsupported_operation"
        ));
        assert_eq!(operations.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn helper_rejects_oversized_unknown_and_invalid_requests_before_calling_operations() {
        let operations = RecordingOperations::default();
        let response = handle_request(&vec![b' '; MAX_RPC_REQUEST_BYTES + 1], &operations);
        assert!(matches!(
            response.result,
            ResponseResult::Error(ResponseError { ref code, .. }) if code == "request_too_large"
        ));

        let mut value = serde_json::to_value(apply_request()).unwrap();
        value["payload"]["caller_path"] = json!("/tmp/attacker.conf");
        let response = handle_request(&serde_json::to_vec(&value).unwrap(), &operations);
        assert!(matches!(response.result, ResponseResult::Error(_)));

        let mut request = apply_request();
        request.version = 9;
        let response = handle_request(&serde_json::to_vec(&request).unwrap(), &operations);
        assert!(matches!(response.result, ResponseResult::Error(_)));
        assert!(operations.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn helper_revalidates_domain_and_document_root_before_filesystem_operations() {
        let operations = RecordingOperations::default();
        let mut request = apply_request();
        request.operation = Operation::SiteApply(SiteApplyPayload {
            site_id: Uuid::new_v4(),
            domain: "example.test".to_string(),
            spec: SiteSpec::Static {
                document_root: "/srv/fusionpanel/sites/example.test/../other".to_string(),
            },
        });
        let response = handle_request(&serde_json::to_vec(&request).unwrap(), &operations);
        assert!(matches!(response.result, ResponseResult::Error(_)));
        assert!(operations.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn helper_reports_uncertain_rollback_with_a_safe_error_code_and_message() {
        let operations = RecordingOperations::default();
        *operations.failure.lock().unwrap() = Some(ManagedSiteError::RollbackFailed);
        let response = handle_request(&serde_json::to_vec(&apply_request()).unwrap(), &operations);
        match response.result {
            ResponseResult::Error(error) => {
                assert_eq!(error.code, "rollback_failed");
                assert!(error.message.contains("manual recovery"));
                assert!(!error.message.contains("/etc/nginx"));
            }
            _ => panic!("rollback failure must not return success"),
        }
    }

    #[test]
    fn bound_socket_is_group_accessible_without_following_existing_symlinks() {
        let directory = tempdir().unwrap();
        let socket = directory.path().join("helper.sock");
        let listener = bind_socket(&socket).unwrap();
        let permissions = fs::metadata(&socket).unwrap().permissions().mode() & 0o777;
        assert_eq!(permissions, 0o660);
        drop(listener);
        fs::remove_file(&socket).unwrap();

        let target = directory.path().join("target");
        fs::write(&target, "preserve").unwrap();
        std::os::unix::fs::symlink(&target, &socket).unwrap();
        assert_eq!(
            bind_socket(&socket).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "preserve");
    }

    #[test]
    fn privileged_helper_rejects_writable_managed_paths_and_external_symlink_targets() {
        let directory = tempdir().unwrap();
        let managed_root = directory.path().join("etc/nginx/fusionpanel");
        for child in ["sites-available", "sites-enabled", "staging"] {
            fs::create_dir_all(managed_root.join(child)).unwrap();
        }
        for path in [
            directory.path().join("etc"),
            directory.path().join("etc/nginx"),
            managed_root.clone(),
            managed_root.join("sites-available"),
            managed_root.join("sites-enabled"),
            managed_root.join("staging"),
        ] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o750)).unwrap();
        }
        let expected_uid = fs::metadata(directory.path()).unwrap().uid();
        for path in [
            directory.path().join("etc"),
            directory.path().join("etc/nginx"),
            managed_root.clone(),
            managed_root.join("sites-available"),
            managed_root.join("sites-enabled"),
            managed_root.join("staging"),
        ] {
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(metadata.is_dir(), "not a directory: {path:?}");
            assert!(
                !metadata.file_type().is_symlink(),
                "unexpected symlink: {path:?}"
            );
            assert_eq!(metadata.uid(), expected_uid, "unexpected owner: {path:?}");
            assert_eq!(
                metadata.permissions().mode() & 0o022,
                0,
                "writable by group or other: {path:?}"
            );
            validate_owned_directory(&path, expected_uid, false)
                .unwrap_or_else(|error| panic!("unsafe test path {path:?}: {error}"));
        }
        validate_managed_config_tree(&managed_root, expected_uid).unwrap();

        let enabled = managed_root.join("sites-enabled");
        fs::set_permissions(&enabled, fs::Permissions::from_mode(0o770)).unwrap();
        assert_eq!(
            validate_managed_config_tree(&managed_root, expected_uid),
            Err(ManagedSiteError::UnsafePath)
        );
        fs::set_permissions(&enabled, fs::Permissions::from_mode(0o750)).unwrap();

        let config = managed_root.join("sites-available/site.conf");
        fs::write(&config, "server {}\n").unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o660)).unwrap();
        assert_eq!(
            validate_managed_config_tree(&managed_root, expected_uid),
            Err(ManagedSiteError::UnsafePath)
        );
        fs::set_permissions(&config, fs::Permissions::from_mode(0o640)).unwrap();

        let outside = directory.path().join("outside.conf");
        fs::write(&outside, "server {}\n").unwrap();
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o640)).unwrap();
        std::os::unix::fs::symlink(&outside, enabled.join("example.test.conf")).unwrap();
        assert_eq!(
            validate_managed_config_tree(&managed_root, expected_uid),
            Err(ManagedSiteError::UnsafePath)
        );
    }

    #[test]
    fn client_uses_the_bounded_local_rpc_and_preserves_typed_errors() {
        let directory = tempdir().unwrap();
        let socket = directory.path().join("helper.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let thread = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            serve_connection(stream, &RecordingOperations::default());
        });
        let client = NginxHelperClient::new(socket);
        let result = client.apply_site(
            Uuid::new_v4(),
            "example.test",
            &ManagedSiteSpec::Static {
                document_root: "/srv/fusionpanel/sites/example.test".to_string(),
            },
        );
        assert_eq!(result, Ok(()));
        thread.join().unwrap();
    }
}
