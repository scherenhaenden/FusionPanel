use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug)]
pub struct AgentRequest {
    pub version: u32,
    pub request_id: String,
    pub operation: String,
    pub payload: Value,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AgentResponse {
    pub version: u32,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AgentError>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AgentError {
    pub code: String,
    pub message: String,
}

pub fn handle_request(req: AgentRequest) -> AgentResponse {
    if req.version != 1 {
        return AgentResponse {
            version: req.version,
            request_id: req.request_id,
            result: None,
            error: Some(AgentError {
                code: "unsupported_version".to_string(),
                message: "Only version 1 is supported".to_string(),
            }),
        };
    }

    match req.operation.as_str() {
        "system.info" => AgentResponse {
            version: 1,
            request_id: req.request_id,
            result: Some(serde_json::json!({
                "os": "fusionpanel-os",
                "uptime": 12345
            })),
            error: None,
        },
        "site.validate" => {
            let domain = req.payload.get("domain").and_then(|v| v.as_str()).unwrap_or("");
            let docroot = req.payload.get("document_root").and_then(|v| v.as_str()).unwrap_or("");

            if domain.is_empty() || docroot.is_empty() {
                return AgentResponse {
                    version: 1,
                    request_id: req.request_id,
                    result: None,
                    error: Some(AgentError {
                        code: "invalid_input".to_string(),
                        message: "domain and document_root are required".to_string(),
                    }),
                };
            }

            if domain.contains("..") || docroot.contains("..") || docroot.contains("\0") || !docroot.starts_with("/srv/fusionpanel/sites/") {
                 return AgentResponse {
                    version: 1,
                    request_id: req.request_id,
                    result: None,
                    error: Some(AgentError {
                        code: "invalid_path".to_string(),
                        message: "Invalid domain or document_root".to_string(),
                    }),
                };
            }

            AgentResponse {
                version: 1,
                request_id: req.request_id,
                result: Some(serde_json::json!({
                    "valid": true
                })),
                error: None,
            }
        },
        _ => AgentResponse {
            version: 1,
            request_id: req.request_id,
            result: None,
            error: Some(AgentError {
                code: "unsupported_operation".to_string(),
                message: "Unsupported operation".to_string(),
            }),
        }
    }
}
