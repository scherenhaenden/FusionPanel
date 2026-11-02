#![forbid(unsafe_code)]

use reqwest::Client;
use std::fmt;

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

        Ok(Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            bucket: bucket.to_string(),
            credentials,
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

    /// Multipart upload using chunking and retry for transient network errors.
    pub async fn upload_multipart(&self, key: &str, data: &[u8]) -> Result<(), S3Error> {
        let client = Client::new();
        let url = format!("{}/{}/{}?uploads", self.endpoint, self.bucket, key);

        // Note: For a complete solution, AWS SigV4 signing must be applied to all requests.
        // For the purpose of this adaptation, we execute the retry loop over HTTP calls.

        // 1. Initiate Multipart Upload
        let mut retries = 0;
        let max_retries = 3;

        let upload_id = loop {
            let res = client.post(&url).send().await;

            match res {
                Ok(response) if response.status().is_success() => {
                    // MOCK parsing the XML response to get UploadId
                    break "mock-upload-id".to_string();
                }
                _ => {
                    retries += 1;
                    if retries >= max_retries {
                        return Err(S3Error::NetworkError("Failed to initiate upload".into()));
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        };

        // 2. Upload Parts
        let total_size = data.len() as u64;
        let chunk_size = 5 * 1024 * 1024; // 5MB minimum
        let chunks = Self::calculate_chunks(total_size, chunk_size);

        let mut completed_parts = Vec::new();
        for (part_number, (offset, length)) in chunks.into_iter().enumerate() {
            let part_number = part_number + 1;
            let chunk_data = &data[offset as usize..(offset + length) as usize];
            let part_url = format!(
                "{}/{}/{}?partNumber={}&uploadId={}",
                self.endpoint, self.bucket, key, part_number, upload_id
            );

            let mut part_retries = 0;
            let etag = loop {
                let res = client.put(&part_url).body(chunk_data.to_vec()).send().await;

                match res {
                    Ok(response) if response.status().is_success() => {
                        let etag = response
                            .headers()
                            .get("ETag")
                            .and_then(|h| h.to_str().ok())
                            .unwrap_or("\"mock-etag\"")
                            .to_string();
                        break etag;
                    }
                    _ => {
                        part_retries += 1;
                        if part_retries >= max_retries {
                            return Err(S3Error::UploadFailed);
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                }
            };

            completed_parts.push((part_number, etag));
        }

        // 3. Complete Multipart Upload
        let complete_url = format!(
            "{}/{}/{}?uploadId={}",
            self.endpoint, self.bucket, key, upload_id
        );
        // Build XML body
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
            let res = client
                .post(&complete_url)
                .body(xml_body.clone())
                .send()
                .await;

            match res {
                Ok(response) if response.status().is_success() => {
                    break;
                }
                _ => {
                    complete_retries += 1;
                    if complete_retries >= max_retries {
                        return Err(S3Error::NetworkError("Failed to complete upload".into()));
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }

        Ok(())
    }
}
