use sha2::{Digest, Sha256};
use hmac::{Hmac, Mac};
use hex;
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use reqwest::header::HeaderMap;

type HmacSha256 = Hmac<Sha256>;

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    let result = mac.finalize();
    let bytes = result.into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    out
}

pub fn sha256_hex(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    hex::encode(hash)
}

pub struct SigV4Request<'a> {
    pub method: &'a str,
    pub endpoint: &'a str,
    pub headers: BTreeMap<String, String>,
    pub body: &'a [u8],
    pub region: &'a str,
    pub access_key: &'a str,
    pub secret_key: &'a str,
    pub datetime: DateTime<Utc>,
}

pub fn sign(req: &mut SigV4Request) -> HeaderMap {
    let amz_date = req.datetime.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = req.datetime.format("%Y%m%d").to_string();
    let service = "s3";

    req.headers.insert("x-amz-date".to_string(), amz_date.clone());

    let url_parts: Vec<&str> = req.endpoint.splitn(2, "://").collect();
    let without_scheme = url_parts.get(1).unwrap_or(&req.endpoint);
    let path_and_query: Vec<&str> = without_scheme.splitn(2, '/').collect();

    // Fix: Keep the port in the Host header for MinIO/custom ports
    let host = path_and_query[0];
    req.headers.insert("host".to_string(), host.to_string());

    let path_query = if path_and_query.len() > 1 {
        format!("/{}", path_and_query[1])
    } else {
        "/".to_string()
    };

    let (path, query) = if let Some(idx) = path_query.find('?') {
        (&path_query[..idx], &path_query[idx+1..])
    } else {
        (path_query.as_str(), "")
    };

    let mut query_params: Vec<(&str, &str)> = query.split('&').filter(|s| !s.is_empty()).map(|kv| {
        let mut parts = kv.splitn(2, '=');
        (parts.next().unwrap(), parts.next().unwrap_or(""))
    }).collect();
    query_params.sort_by(|a, b| a.0.cmp(b.0));

    let canonical_querystring = query_params.iter().map(|(k, v)| {
        let k = urlencoding::encode(k);
        let v = urlencoding::encode(v);
        format!("{}={}", k, v)
    }).collect::<Vec<String>>().join("&");

    let canonical_uri = path.split('/').map(|s| urlencoding::encode(s).into_owned()).collect::<Vec<String>>().join("/");
    let canonical_uri = if canonical_uri.is_empty() { "/".to_string() } else { canonical_uri };

    let payload_hash = sha256_hex(req.body);
    req.headers.insert("x-amz-content-sha256".to_string(), payload_hash.clone());

    let mut signed_headers = String::new();
    let mut canonical_headers = String::new();
    for (k, v) in &req.headers {
        let k_lower = k.to_lowercase();
        canonical_headers.push_str(&format!("{}:{}\n", k_lower, v.trim()));
        signed_headers.push_str(&format!("{};", k_lower));
    }
    signed_headers.pop();

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        req.method,
        canonical_uri,
        canonical_querystring,
        canonical_headers,
        signed_headers,
        payload_hash
    );

    let credential_scope = format!("{}/{}/{}/aws4_request", date_stamp, req.region, service);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date,
        credential_scope,
        sha256_hex(canonical_request.as_bytes())
    );

    let k_date = hmac_sha256(format!("AWS4{}", req.secret_key).as_bytes(), date_stamp.as_bytes());
    let k_region = hmac_sha256(&k_date, req.region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()));

    let auth_header = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        req.access_key, credential_scope, signed_headers, signature
    );

    req.headers.insert("authorization".to_string(), auth_header);

    let mut header_map = HeaderMap::new();
    for (k, v) in &req.headers {
        if let Ok(h_name) = reqwest::header::HeaderName::from_bytes(k.as_bytes()) {
            if let Ok(h_val) = reqwest::header::HeaderValue::from_str(v) {
                header_map.insert(h_name, h_val);
            }
        }
    }

    header_map
}
