#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
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
}

impl Request {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.version != 1 {
            return Err(ValidationError::UnsupportedVersion);
        }
        match &self.operation {
            Operation::SystemInfo(_) => Ok(()),
            Operation::SiteValidate(payload) => payload.validate(),
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
