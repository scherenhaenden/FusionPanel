#![forbid(unsafe_code)]

//! Foundation implementation pending assigned work package.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub request_id: Uuid,
    #[serde(flatten)]
    pub operation: Operation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", content = "payload")]
pub enum Operation {
    #[serde(rename = "system.info")]
    SystemInfo(SystemInfoPayload),
    #[serde(rename = "site.validate")]
    SiteValidate(SiteValidatePayload),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SystemInfoPayload {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SiteValidatePayload {
    pub domain: String,
    pub document_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub version: u32,
    pub request_id: Uuid,
    #[serde(flatten)]
    pub result: ResponseResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ResponseResult {
    Result(serde_json::Value),
    Error(ErrorObject),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ErrorObject {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    UnsupportedVersion,
    InvalidDomain,
    InvalidDocumentRoot,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion => write!(f, "unsupported version"),
            Self::InvalidDomain => write!(f, "invalid domain"),
            Self::InvalidDocumentRoot => write!(f, "invalid document root"),
        }
    }
}

impl std::error::Error for ValidationError {}

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
        // Validate domain
        if self.domain.is_empty()
            || self.domain.contains('\0')
            || self.domain.contains('/')
            || self.domain.contains('\\')
            || self.domain.contains("..")
        {
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
