use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentRequest {
    pub version: u32,
    pub request_id: String,
    pub operation: String,
    pub payload: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentResponse {
    pub version: u32,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AgentError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentError {
    pub code: String,
    pub message: String,
}
