#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
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
}
