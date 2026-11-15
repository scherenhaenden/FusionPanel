#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum PresignedUrlError {
    #[error("Expiration must be between 1 second and 7 days (604800 seconds)")]
    InvalidExpiration,
}

pub struct PresignedUrlGenerator {
    access_key: String,
    secret_key: String,
    region: String,
    endpoint: String,
}

impl PresignedUrlGenerator {
    pub fn new(access_key: &str, secret_key: &str, region: &str, endpoint: &str) -> Self {
        Self {
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
            region: region.to_string(),
            endpoint: endpoint.trim_end_matches('/').to_string(),
        }
    }

    pub fn generate(
        &self,
        method: &str,
        bucket: &str,
        key: &str,
        expires_in: Duration,
        now: DateTime<Utc>,
    ) -> Result<String, PresignedUrlError> {
        let secs = expires_in.as_secs();
        if secs < 1 || secs > 604800 {
            return Err(PresignedUrlError::InvalidExpiration);
        }

        let datestamp = now.format("%Y%m%d").to_string();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();

        let credential_scope = format!("{}/{}/s3/aws4_request", datestamp, self.region);

        let encoded_key = key.split('/')
            .map(|part| urlencoding::encode(part).into_owned())
            .collect::<Vec<_>>()
            .join("/");

        let mut query_params = vec![
            ("X-Amz-Algorithm", "AWS4-HMAC-SHA256".to_string()),
            ("X-Amz-Credential", format!("{}/{}", self.access_key, credential_scope)),
            ("X-Amz-Date", amz_date.clone()),
            ("X-Amz-Expires", secs.to_string()),
            ("X-Amz-SignedHeaders", "host".to_string()),
        ];

        query_params.sort_by(|a, b| a.0.cmp(&b.0));

        let canonical_querystring = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let host = if self.endpoint.contains("://") {
            let parts: Vec<&str> = self.endpoint.split("://").collect();
            parts.get(1).unwrap_or(&self.endpoint.as_str()).to_string()
        } else {
            self.endpoint.clone()
        };

        let canonical_uri = format!("/{}/{}", bucket, encoded_key);

        let canonical_headers = format!("host:{}\n", host);
        let signed_headers = "host";

        let payload_hash = "UNSIGNED-PAYLOAD";

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method, canonical_uri, canonical_querystring, canonical_headers, signed_headers, payload_hash
        );

        let mut hasher = Sha256::new();
        hasher.update(canonical_request.as_bytes());
        let canonical_request_hash = hex::encode(hasher.finalize());

        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date, credential_scope, canonical_request_hash
        );

        let signing_key = self.get_signature_key(&self.secret_key, &datestamp, &self.region, "s3");
        let signature = hex::encode(Self::sign(&signing_key, string_to_sign.as_bytes()));

        let final_url = format!(
            "{}{}?{}&X-Amz-Signature={}",
            self.endpoint, canonical_uri, canonical_querystring, signature
        );

        Ok(final_url)
    }

    fn sign(key: &[u8], msg: &[u8]) -> Vec<u8> {
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
        mac.update(msg);
        mac.finalize().into_bytes().to_vec()
    }

    fn get_signature_key(&self, key: &str, date_stamp: &str, region_name: &str, service_name: &str) -> Vec<u8> {
        let k_secret = format!("AWS4{}", key);
        let k_date = Self::sign(k_secret.as_bytes(), date_stamp.as_bytes());
        let k_region = Self::sign(&k_date, region_name.as_bytes());
        let k_service = Self::sign(&k_region, service_name.as_bytes());
        Self::sign(&k_service, b"aws4_request")
    }
}
