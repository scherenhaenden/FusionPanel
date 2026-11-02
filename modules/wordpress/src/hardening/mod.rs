use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ChecksumsResponse {
    pub checksums: HashMap<String, String>,
}

pub struct HardeningOptions {
    pub disable_xmlrpc: bool,
    pub disallow_file_edit: bool,
}

pub fn generate_wp_config_hardening(options: &HardeningOptions) -> String {
    let mut config_lines = Vec::new();

    if options.disallow_file_edit {
        config_lines.push("define( 'DISALLOW_FILE_EDIT', true );");
    }

    config_lines.join("\n")
}

pub fn generate_htaccess_hardening(options: &HardeningOptions) -> String {
    let mut rules = Vec::new();

    if options.disable_xmlrpc {
        rules.push("<Files xmlrpc.php>");
        rules.push("Order Allow,Deny");
        rules.push("Deny from all");
        rules.push("</Files>");
    }

    rules.join("\n")
}

#[derive(Debug, PartialEq)]
pub enum ChecksumError {
    RequestFailed(String),
    ParseFailed(String),
}

pub async fn fetch_core_checksums_with_client(
    client: &reqwest::Client,
    base_url: &str,
    version: &str,
    locale: &str,
) -> Result<ChecksumsResponse, ChecksumError> {
    let url = format!(
        "{}/core/checksums/1.0/?version={}&locale={}",
        base_url, version, locale
    );
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| ChecksumError::RequestFailed(e.to_string()))?;

    if !response.status().is_success() {
        return Err(ChecksumError::RequestFailed(format!(
            "Failed to fetch checksums, status: {}",
            response.status()
        )));
    }

    let parsed = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| ChecksumError::ParseFailed(e.to_string()))?;

    if let Some(checksums_obj) = parsed.get("checksums").and_then(|c| c.as_object()) {
        let mut checksums = HashMap::new();
        // The API actually returns nested versions like {"1.0": ..., "5.8": ...} OR {"wp-admin/...": "..."}
        // According to https://api.wordpress.org/core/checksums/1.0/?version=6.0
        // The response format is typically: {"checksums": {"6.0": {"wp-admin/about.php": "..."}}}
        // However, usually it is requested with a specific version and sometimes returns just that version as the key inside checksums.
        // Let's handle both "checksums": {"wp-admin/...": "..."} and "checksums": {"VERSION": {"wp-admin/...": "..."}}

        let mut found_file_hashes = false;

        // check if it's nested under the version key
        if let Some(version_obj) = checksums_obj.get(version).and_then(|v| v.as_object()) {
            for (k, v) in version_obj {
                if let Some(val) = v.as_str() {
                    checksums.insert(k.clone(), val.to_string());
                    found_file_hashes = true;
                }
            }
        }

        // If not nested under version, assume flat structure
        if !found_file_hashes {
            for (k, v) in checksums_obj {
                if let Some(val) = v.as_str() {
                    checksums.insert(k.clone(), val.to_string());
                }
            }
        }

        Ok(ChecksumsResponse { checksums })
    } else {
        Err(ChecksumError::ParseFailed(
            "Invalid JSON structure: missing 'checksums' object".into(),
        ))
    }
}

pub async fn fetch_core_checksums(
    version: &str,
    locale: &str,
) -> Result<ChecksumsResponse, ChecksumError> {
    let client = reqwest::Client::new();
    fetch_core_checksums_with_client(&client, "https://api.wordpress.org", version, locale).await
}
