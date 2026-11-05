#![forbid(unsafe_code)]

pub mod sigv4;

use std::fmt;
use std::collections::BTreeMap;
use chrono::Utc;
use crate::s3::sigv4::{SigV4Request, sign};
use std::time::Duration;

/// Credentials for S3 storage
#[derive(Clone)]
pub struct S3Credentials {
    pub access_key: String,
    pub secret_key: String,
}

impl fmt::Debug for S3Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3Credentials")
            .field("access_key", &"***REDACTED***")
            .field("secret_key", &"***REDACTED***")
            .finish()
    }
}

impl fmt::Display for S3Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "S3Credentials(***REDACTED***)")
    }
}

#[derive(Debug, Clone)]
pub struct S3StorageBackend {
    pub endpoint: String,
    pub bucket: String,
    pub credentials: S3Credentials,
    pub region: String,
    pub client: reqwest::Client,
}

#[derive(Debug)]
pub enum S3Error {
    InvalidEndpoint,
    InvalidBucket,
    UploadFailed,
    NetworkError(String),
}

impl fmt::Display for S3Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            S3Error::InvalidEndpoint => write!(f, "Invalid endpoint URL"),
            S3Error::InvalidBucket => write!(f, "Invalid bucket name"),
            S3Error::UploadFailed => write!(f, "Upload failed after retries"),
            S3Error::NetworkError(e) => write!(f, "Network error: {}", e),
        }
    }
}

impl std::error::Error for S3Error {}

impl S3StorageBackend {
    pub fn new(endpoint: &str, bucket: &str, credentials: S3Credentials) -> Result<Self, S3Error> {
        if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
            return Err(S3Error::InvalidEndpoint);
        }

        // Basic url validation
        if !endpoint.contains('.') || endpoint.contains(' ') {
            return Err(S3Error::InvalidEndpoint);
        }

        // Validate bucket name (simple rules for AWS/R2/MinIO)
        if bucket.len() < 3 || bucket.len() > 63 {
            return Err(S3Error::InvalidBucket);
        }
        if !bucket
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
        {
            return Err(S3Error::InvalidBucket);
        }

        let endpoint = endpoint.trim_end_matches('/');

        // Very basic region extraction
        let region = if endpoint.contains("amazonaws.com") {
            if let Some(r) = endpoint.strip_prefix("https://s3.") {
                if let Some(idx) = r.find(".amazonaws.com") {
                    let extracted = &r[..idx];
                    if !extracted.is_empty() {
                        extracted.to_string()
                    } else {
                        "us-east-1".to_string()
                    }
                } else {
                    "us-east-1".to_string()
                }
            } else if let Some(r) = endpoint.strip_prefix("https://s3-") {
                if let Some(idx) = r.find(".amazonaws.com") {
                    r[..idx].to_string()
                } else {
                    "us-east-1".to_string()
                }
            } else {
                "us-east-1".to_string()
            }
        } else {
            "us-east-1".to_string()
        };

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| S3Error::NetworkError(e.to_string()))?;

        Ok(Self {
            endpoint: endpoint.to_string(),
            bucket: bucket.to_string(),
            credentials,
            region,
            client,
        })
    }

    pub fn calculate_chunks(total_size: u64, chunk_size: u64) -> Vec<(u64, u64)> {
        let min_chunk = 5 * 1024 * 1024;
        let actual_chunk_size = chunk_size.max(min_chunk);

        let mut chunks = Vec::new();
        let mut offset = 0;

        while offset < total_size {
            let length = if offset + actual_chunk_size > total_size {
                total_size - offset
            } else {
                actual_chunk_size
            };
            chunks.push((offset, length));
            offset += length;
        }

        if chunks.is_empty() && total_size == 0 {
            chunks.push((0, 0));
        }

        chunks
    }

    async fn request(
        &self,
        method: &str,
        key: &str,
        query: &str,
        body: &[u8],
    ) -> Result<reqwest::Response, S3Error> {
        let encoded_key = key.split('/').map(|s| urlencoding::encode(s).into_owned()).collect::<Vec<String>>().join("/");

        let url = if query.is_empty() {
            format!("{}/{}/{}", self.endpoint, self.bucket, encoded_key)
        } else {
            format!("{}/{}/{}?{}", self.endpoint, self.bucket, encoded_key, query)
        };

        let mut sig_req = SigV4Request {
            method,
            endpoint: &url,
            headers: BTreeMap::new(),
            body,
            region: &self.region,
            access_key: &self.credentials.access_key,
            secret_key: &self.credentials.secret_key,
            datetime: Utc::now(),
        };

        let headers = sign(&mut sig_req);

        let req_method = match method {
            "GET" => reqwest::Method::GET,
            "PUT" => reqwest::Method::PUT,
            "POST" => reqwest::Method::POST,
            "DELETE" => reqwest::Method::DELETE,
            "HEAD" => reqwest::Method::HEAD,
            _ => reqwest::Method::GET,
        };

        let res = self.client
            .request(req_method, &url)
            .headers(headers)
            .body(body.to_vec())
            .send()
            .await;

        match res {
            Ok(response) => Ok(response),
            Err(e) => Err(S3Error::NetworkError(e.to_string())),
        }
    }

    /// Abort multipart upload
    async fn abort_multipart(&self, key: &str, upload_id: &str) -> Result<(), S3Error> {
        let query = format!("uploadId={}", upload_id);
        let res = self.request("DELETE", key, &query, b"").await?;

        if res.status().is_success() {
            Ok(())
        } else {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            Err(S3Error::NetworkError(format!("Abort failed: {} - {}", status, text)))
        }
    }

    pub async fn upload_multipart(&self, key: &str, data: &[u8]) -> Result<(), S3Error> {
        // 1. Initiate Multipart Upload
        let mut retries = 0;
        let max_retries = 3;

        let upload_id = loop {
            match self.request("POST", key, "uploads", b"").await {
                Ok(res) if res.status().is_success() => {
                    let text = res.text().await.unwrap_or_default();
                    // Very simple XML parse for UploadId
                    if let Some(start) = text.find("<UploadId>") {
                        let text_after = &text[start + 10..];
                        if let Some(end) = text_after.find("</UploadId>") {
                            break text_after[..end].to_string();
                        }
                    }
                    return Err(S3Error::NetworkError("Failed to parse UploadId".into()));
                }
                Ok(res) => {
                    retries += 1;
                    if retries >= max_retries {
                        let status = res.status();
                        let text = res.text().await.unwrap_or_default();
                        return Err(S3Error::NetworkError(format!("Failed to initiate upload: {} - {}", status, text)));
                    }
                }
                Err(e) => {
                    retries += 1;
                    if retries >= max_retries {
                        return Err(e);
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        };

        // 2. Upload Parts
        let total_size = data.len() as u64;
        let chunk_size = 5 * 1024 * 1024; // 5MB minimum
        let chunks = Self::calculate_chunks(total_size, chunk_size);

        let mut completed_parts = Vec::new();
        for (part_number, (offset, length)) in chunks.into_iter().enumerate() {
            let part_number = part_number + 1;
            let chunk_data = &data[offset as usize..(offset + length) as usize];
            let query = format!("partNumber={}&uploadId={}", part_number, upload_id);

            let mut part_retries = 0;
            let etag = loop {
                match self.request("PUT", key, &query, chunk_data).await {
                    Ok(res) if res.status().is_success() => {
                        if let Some(etag) = res.headers().get("ETag") {
                            break etag.to_str().unwrap_or("").to_string();
                        }
                        // ETag missing
                        break "\"\"".to_string();
                    }
                    Ok(_) | Err(_) => {
                        part_retries += 1;
                        if part_retries >= max_retries {
                            // Abort and fail
                            let _ = self.abort_multipart(key, &upload_id).await;
                            return Err(S3Error::UploadFailed);
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            };

            completed_parts.push((part_number, etag));
        }

        // 3. Complete Multipart Upload
        let complete_query = format!("uploadId={}", upload_id);
        let mut xml_body = String::from("<CompleteMultipartUpload>");
        for (part, etag) in completed_parts {
            xml_body.push_str(&format!(
                "<Part><PartNumber>{}</PartNumber><ETag>{}</ETag></Part>",
                part, etag
            ));
        }
        xml_body.push_str("</CompleteMultipartUpload>");

        let mut complete_retries = 0;
        loop {
            match self.request("POST", key, &complete_query, xml_body.as_bytes()).await {
                Ok(res) if res.status().is_success() => {
                    // Ensure the server identifier XML is valid, indicating success.
                    let text = res.text().await.unwrap_or_default();
                    if text.contains("<CompleteMultipartUploadResult") || text.contains("<Error>") == false {
                         break;
                    }
                    // if it contains error, we retry.
                }
                Ok(res) => {
                    complete_retries += 1;
                    if complete_retries >= max_retries {
                        let status = res.status();
                        let text = res.text().await.unwrap_or_default();
                        let _ = self.abort_multipart(key, &upload_id).await;
                        return Err(S3Error::NetworkError(format!("Failed to complete upload: {} - {}", status, text)));
                    }
                }
                Err(e) => {
                    complete_retries += 1;
                    if complete_retries >= max_retries {
                        let _ = self.abort_multipart(key, &upload_id).await;
                        return Err(e);
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Ok(())
    }
}

use async_trait::async_trait;
use crate::traits::{StorageBackend, StorageError, FileStat};

#[async_trait]
impl StorageBackend for S3StorageBackend {
    async fn upload(&self, path: &str, data: Vec<u8>) -> Result<(), StorageError> {
        let chunk_size = 5 * 1024 * 1024;
        if data.len() < chunk_size {
            let mut retries = 0;
            loop {
                match self.request("PUT", path, "", &data).await {
                    Ok(res) if res.status().is_success() => return Ok(()),
                    Ok(res) => {
                        retries += 1;
                        if retries >= 3 {
                            let text = res.text().await.unwrap_or_default();
                            return Err(StorageError::Other(format!("Upload failed: {}", text)));
                        }
                    }
                    Err(_e) => {
                        retries += 1;
                        if retries >= 3 {
                            return Err(StorageError::NetworkTimeout);
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        } else {
            self.upload_multipart(path, &data).await.map_err(|e| match e {
                S3Error::NetworkError(_) => StorageError::NetworkTimeout,
                S3Error::UploadFailed => StorageError::Other("Upload Failed".into()),
                _ => StorageError::Other(e.to_string()),
            })
        }
    }

    async fn download(&self, path: &str) -> Result<Vec<u8>, StorageError> {
        match self.request("GET", path, "", b"").await {
            Ok(res) if res.status().is_success() => {
                let bytes = res.bytes().await.map_err(|e| StorageError::Other(e.to_string()))?;
                Ok(bytes.to_vec())
            }
            Ok(res) if res.status() == reqwest::StatusCode::NOT_FOUND => Err(StorageError::NotFound),
            Ok(res) if res.status() == reqwest::StatusCode::FORBIDDEN => Err(StorageError::PermissionDenied),
            Ok(res) => Err(StorageError::Other(format!("Download failed: {}", res.status()))),
            Err(_) => Err(StorageError::NetworkTimeout),
        }
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        match self.request("DELETE", path, "", b"").await {
            Ok(res) if res.status().is_success() || res.status() == reqwest::StatusCode::NO_CONTENT => Ok(()),
            Ok(res) if res.status() == reqwest::StatusCode::NOT_FOUND => Err(StorageError::NotFound),
            Ok(res) if res.status() == reqwest::StatusCode::FORBIDDEN => Err(StorageError::PermissionDenied),
            Ok(res) => Err(StorageError::Other(format!("Delete failed: {}", res.status()))),
            Err(_) => Err(StorageError::NetworkTimeout),
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        let query = if prefix.is_empty() {
            "list-type=2".to_string()
        } else {
            format!("list-type=2&prefix={}", urlencoding::encode(prefix))
        };

        match self.request("GET", "", &query, b"").await {
            Ok(res) if res.status().is_success() => {
                let text = res.text().await.map_err(|e| StorageError::Other(e.to_string()))?;
                let mut results = Vec::new();

                // Very simple XML parse for Keys
                let mut current_idx = 0;
                while let Some(start) = text[current_idx..].find("<Key>") {
                    let key_start = current_idx + start + 5;
                    if let Some(end) = text[key_start..].find("</Key>") {
                        let key_end = key_start + end;
                        results.push(text[key_start..key_end].to_string());
                        current_idx = key_end + 6;
                    } else {
                        break;
                    }
                }
                Ok(results)
            }
            Ok(res) if res.status() == reqwest::StatusCode::FORBIDDEN => Err(StorageError::PermissionDenied),
            Ok(res) => Err(StorageError::Other(format!("List failed: {}", res.status()))),
            Err(_) => Err(StorageError::NetworkTimeout),
        }
    }

    async fn stat(&self, path: &str) -> Result<FileStat, StorageError> {
        match self.request("HEAD", path, "", b"").await {
            Ok(res) if res.status().is_success() => {
                let size = res.headers()
                    .get(reqwest::header::CONTENT_LENGTH)
                    .and_then(|val| val.to_str().ok())
                    .and_then(|val| val.parse::<u64>().ok())
                    .unwrap_or(0);
                Ok(FileStat { size })
            }
            Ok(res) if res.status() == reqwest::StatusCode::NOT_FOUND => Err(StorageError::NotFound),
            Ok(res) if res.status() == reqwest::StatusCode::FORBIDDEN => Err(StorageError::PermissionDenied),
            Ok(res) => Err(StorageError::Other(format!("Stat failed: {}", res.status()))),
            Err(_) => Err(StorageError::NetworkTimeout),
        }
    }
}
