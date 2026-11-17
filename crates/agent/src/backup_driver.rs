#![forbid(unsafe_code)]

use chrono::Utc;
use fusionpanel_agent_protocol::{
    BackupCreatePayload, BackupDestination, BackupRestorePayload, BackupTarget, DatabaseEngineKind,
};
use hmac::{Hmac, Mac};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tokio::process::Command;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Serialize, Deserialize)]
struct BackupManifest {
    version: u32,
    site_tar_gz_sha256: Option<String>,
    db_sql_sha256: Option<String>,
}

pub async fn execute_backup_create(payload: &BackupCreatePayload) -> Result<String, String> {
    let temp = TempDir::new().map_err(|e| format!("failed to create staging dir: {e}"))?;
    let staging = temp.path().join("bundle");
    fs::create_dir_all(&staging).map_err(|e| format!("failed to create bundle dir: {e}"))?;

    let mut manifest = BackupManifest {
        version: 1,
        site_tar_gz_sha256: None,
        db_sql_sha256: None,
    };

    match &payload.target {
        BackupTarget::Site { document_root, .. } => {
            let site_tar = staging.join("site.tar.gz");
            archive_directory(Path::new(document_root), &site_tar).await?;
            manifest.site_tar_gz_sha256 = Some(sha256_file(&site_tar)?);
        }
        BackupTarget::Database { name, engine, .. } => {
            let db_sql = staging.join("db.sql");
            dump_database(*engine, name, &db_sql).await?;
            manifest.db_sql_sha256 = Some(sha256_file(&db_sql)?);
        }
        BackupTarget::Bundle {
            document_root,
            database_name,
            engine,
            ..
        } => {
            let site_tar = staging.join("site.tar.gz");
            archive_directory(Path::new(document_root), &site_tar).await?;
            manifest.site_tar_gz_sha256 = Some(sha256_file(&site_tar)?);

            let db_sql = staging.join("db.sql");
            dump_database(*engine, database_name, &db_sql).await?;
            manifest.db_sql_sha256 = Some(sha256_file(&db_sql)?);
        }
    }

    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| format!("manifest encode error: {e}"))?;
    fs::write(staging.join("manifest.json"), manifest_bytes)
        .map_err(|e| format!("failed to write manifest.json: {e}"))?;

    let archive_path = temp.path().join("archive.tar.gz");
    archive_directory(&staging, &archive_path).await?;
    let archive_sha256 = sha256_file(&archive_path)?;

    upload_archive(&payload.destination, &payload.archive_key, &archive_path).await?;
    Ok(archive_sha256)
}

pub async fn execute_backup_restore(payload: &BackupRestorePayload) -> Result<(), String> {
    let temp = TempDir::new().map_err(|e| format!("failed to create restore staging dir: {e}"))?;
    let archive_path = temp.path().join("archive.tar.gz");
    download_archive(&payload.destination, &payload.archive_key, &archive_path).await?;

    if let Some(expected) = &payload.expected_sha256 {
        let actual = sha256_file(&archive_path)?;
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(format!(
                "archive SHA-256 mismatch: expected {expected}, got {actual}"
            ));
        }
    }

    let extract_dir = temp.path().join("extract");
    fs::create_dir_all(&extract_dir).map_err(|e| format!("failed to create extract dir: {e}"))?;
    extract_archive(&archive_path, &extract_dir).await?;

    let manifest_raw = fs::read(extract_dir.join("manifest.json"))
        .map_err(|e| format!("missing or unreadable manifest.json: {e}"))?;
    let manifest: BackupManifest =
        serde_json::from_slice(&manifest_raw).map_err(|e| format!("invalid manifest.json: {e}"))?;
    if manifest.version != 1 {
        return Err(format!("unsupported manifest version {}", manifest.version));
    }

    // Verify staged artifact checksums before mutating any live target.
    if let Some(expected_site_sha) = &manifest.site_tar_gz_sha256 {
        let actual = sha256_file(&extract_dir.join("site.tar.gz"))?;
        if actual != *expected_site_sha {
            return Err("staged site.tar.gz SHA-256 verification failed".to_string());
        }
    }
    if let Some(expected_db_sha) = &manifest.db_sql_sha256 {
        let actual = sha256_file(&extract_dir.join("db.sql"))?;
        if actual != *expected_db_sha {
            return Err("staged db.sql SHA-256 verification failed".to_string());
        }
    }

    match &payload.target {
        BackupTarget::Site { document_root, .. } => {
            restore_directory(&extract_dir.join("site.tar.gz"), Path::new(document_root)).await?;
        }
        BackupTarget::Database { name, engine, .. } => {
            restore_database(*engine, name, &extract_dir.join("db.sql")).await?;
        }
        BackupTarget::Bundle {
            document_root,
            database_name,
            engine,
            ..
        } => {
            restore_directory(&extract_dir.join("site.tar.gz"), Path::new(document_root)).await?;
            restore_database(*engine, database_name, &extract_dir.join("db.sql")).await?;
        }
    }
    Ok(())
}

async fn archive_directory(source_dir: &Path, archive_path: &Path) -> Result<(), String> {
    if !source_dir.is_dir() {
        return Err(format!(
            "backup source directory does not exist: {}",
            source_dir.display()
        ));
    }
    let output = Command::new("tar")
        .arg("-C")
        .arg(source_dir)
        .arg("-czf")
        .arg(archive_path)
        .arg(".")
        .output()
        .await
        .map_err(|e| format!("failed to execute tar: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "tar archive failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

async fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dest_dir).map_err(|e| format!("failed to prepare restore dir: {e}"))?;
    let output = Command::new("tar")
        .arg("-C")
        .arg(dest_dir)
        .arg("-xzf")
        .arg(archive_path)
        .output()
        .await
        .map_err(|e| format!("failed to execute tar extract: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "tar extract failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

async fn restore_directory(site_tar: &Path, document_root: &Path) -> Result<(), String> {
    if !site_tar.is_file() {
        return Err("site.tar.gz missing from backup archive".to_string());
    }
    fs::create_dir_all(document_root)
        .map_err(|e| format!("failed to prepare document root: {e}"))?;
    extract_archive(site_tar, document_root).await
}

async fn dump_database(
    engine: DatabaseEngineKind,
    db_name: &str,
    output_sql: &Path,
) -> Result<(), String> {
    match engine {
        DatabaseEngineKind::Postgresql => {
            let host =
                std::env::var("FUSIONPANEL_PG_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
            let port = std::env::var("FUSIONPANEL_PG_PORT").unwrap_or_else(|_| "5432".to_string());
            let user =
                std::env::var("FUSIONPANEL_PG_USER").unwrap_or_else(|_| "postgres".to_string());
            let pass = std::env::var("FUSIONPANEL_PG_PASSWORD").unwrap_or_default();
            let output = Command::new("pg_dump")
                .env("PGPASSWORD", pass)
                .args([
                    "-h",
                    &host,
                    "-p",
                    &port,
                    "-U",
                    &user,
                    "-d",
                    db_name,
                    "-c",
                    "--if-exists",
                    "-f",
                ])
                .arg(output_sql)
                .output()
                .await
                .map_err(|e| format!("failed to run pg_dump: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "pg_dump failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            Ok(())
        }
        DatabaseEngineKind::Mariadb => {
            let host = std::env::var("FUSIONPANEL_MARIADB_HOST")
                .unwrap_or_else(|_| "127.0.0.1".to_string());
            let port =
                std::env::var("FUSIONPANEL_MARIADB_PORT").unwrap_or_else(|_| "3306".to_string());
            let user =
                std::env::var("FUSIONPANEL_MARIADB_USER").unwrap_or_else(|_| "root".to_string());
            let pass = std::env::var("FUSIONPANEL_MARIADB_PASSWORD").unwrap_or_default();
            let output = Command::new("mariadb-dump")
                .env("MYSQL_PWD", pass)
                .args([
                    &format!("--host={host}"),
                    &format!("--port={port}"),
                    &format!("--user={user}"),
                    "--single-transaction",
                    db_name,
                ])
                .output()
                .await
                .map_err(|e| format!("failed to run mariadb-dump: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "mariadb-dump failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            fs::write(output_sql, &output.stdout)
                .map_err(|e| format!("failed to write db.sql: {e}"))
        }
    }
}

async fn restore_database(
    engine: DatabaseEngineKind,
    db_name: &str,
    input_sql: &Path,
) -> Result<(), String> {
    if !input_sql.is_file() {
        return Err("db.sql missing from backup archive".to_string());
    }
    match engine {
        DatabaseEngineKind::Postgresql => {
            let host =
                std::env::var("FUSIONPANEL_PG_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
            let port = std::env::var("FUSIONPANEL_PG_PORT").unwrap_or_else(|_| "5432".to_string());
            let user =
                std::env::var("FUSIONPANEL_PG_USER").unwrap_or_else(|_| "postgres".to_string());
            let pass = std::env::var("FUSIONPANEL_PG_PASSWORD").unwrap_or_default();
            let output = Command::new("psql")
                .env("PGPASSWORD", pass)
                .args([
                    "-v",
                    "ON_ERROR_STOP=1",
                    "-h",
                    &host,
                    "-p",
                    &port,
                    "-U",
                    &user,
                    "-d",
                    db_name,
                    "-f",
                ])
                .arg(input_sql)
                .output()
                .await
                .map_err(|e| format!("failed to run psql restore: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "psql restore failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            Ok(())
        }
        DatabaseEngineKind::Mariadb => {
            let host = std::env::var("FUSIONPANEL_MARIADB_HOST")
                .unwrap_or_else(|_| "127.0.0.1".to_string());
            let port =
                std::env::var("FUSIONPANEL_MARIADB_PORT").unwrap_or_else(|_| "3306".to_string());
            let user =
                std::env::var("FUSIONPANEL_MARIADB_USER").unwrap_or_else(|_| "root".to_string());
            let pass = std::env::var("FUSIONPANEL_MARIADB_PASSWORD").unwrap_or_default();
            let sql_file = fs::File::open(input_sql)
                .map_err(|e| format!("failed to open db.sql for restore: {e}"))?;
            let output = Command::new("mariadb")
                .env("MYSQL_PWD", pass)
                .args([
                    &format!("--host={host}"),
                    &format!("--port={port}"),
                    &format!("--user={user}"),
                    db_name,
                ])
                .stdin(sql_file)
                .output()
                .await
                .map_err(|e| format!("failed to run mariadb restore: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "mariadb restore failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            Ok(())
        }
    }
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

async fn upload_archive(
    dest: &BackupDestination,
    archive_key: &str,
    archive_path: &Path,
) -> Result<(), String> {
    match dest {
        BackupDestination::Local { base_dir } => {
            let target = PathBuf::from(base_dir).join(archive_key);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("failed to create local backup directory: {e}"))?;
            }
            let tmp = target.with_extension("tar.gz.tmp");
            fs::copy(archive_path, &tmp)
                .map_err(|e| format!("failed to stage local backup archive: {e}"))?;
            fs::rename(&tmp, &target)
                .map_err(|e| format!("failed to commit local backup archive: {e}"))?;
            Ok(())
        }
        BackupDestination::S3 {
            endpoint,
            bucket,
            region,
            access_key,
            secret_key,
            prefix,
        } => {
            let bytes = fs::read(archive_path)
                .map_err(|e| format!("failed to read archive for S3 upload: {e}"))?;
            s3_request(
                Method::PUT,
                endpoint,
                bucket,
                region,
                access_key,
                secret_key,
                prefix.as_deref(),
                archive_key,
                bytes,
            )
            .await?;
            Ok(())
        }
    }
}

async fn download_archive(
    dest: &BackupDestination,
    archive_key: &str,
    out_path: &Path,
) -> Result<(), String> {
    match dest {
        BackupDestination::Local { base_dir } => {
            let source = PathBuf::from(base_dir).join(archive_key);
            fs::copy(&source, out_path)
                .map_err(|e| format!("failed to read local backup {}: {e}", source.display()))?;
            Ok(())
        }
        BackupDestination::S3 {
            endpoint,
            bucket,
            region,
            access_key,
            secret_key,
            prefix,
        } => {
            let bytes = s3_request(
                Method::GET,
                endpoint,
                bucket,
                region,
                access_key,
                secret_key,
                prefix.as_deref(),
                archive_key,
                Vec::new(),
            )
            .await?;
            fs::write(out_path, bytes)
                .map_err(|e| format!("failed to write downloaded S3 archive: {e}"))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn s3_request(
    method: Method,
    endpoint: &str,
    bucket: &str,
    region: &str,
    access_key: &str,
    secret_key: &str,
    prefix: Option<&str>,
    archive_key: &str,
    body: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let base = endpoint.trim_end_matches('/');
    let key = match prefix.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => format!("{}/{}", p.trim_matches('/'), archive_key),
        None => archive_key.to_string(),
    };
    let url = format!("{base}/{bucket}/{key}");
    let parsed = reqwest::Url::parse(&url).map_err(|e| format!("invalid S3 URL: {e}"))?;
    let host = match parsed.port() {
        Some(port) => format!("{}:{port}", parsed.host_str().unwrap_or_default()),
        None => parsed.host_str().unwrap_or_default().to_string(),
    };
    let now = Utc::now();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = now.format("%Y%m%d").to_string();
    let payload_hash = hex::encode(Sha256::digest(&body));

    let auth = sign_aws_v4(
        method.as_str(),
        parsed.path(),
        &host,
        region,
        access_key,
        secret_key,
        &amz_date,
        &date_stamp,
        &payload_hash,
    )?;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("failed to build S3 HTTP client: {e}"))?;
    let mut req = client
        .request(method, &url)
        .header("host", &host)
        .header("x-amz-date", &amz_date)
        .header("x-amz-content-sha256", &payload_hash)
        .header("authorization", auth);
    if !body.is_empty() {
        req = req.body(body);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("S3 request failed: {e}"))?;
    let status = resp.status();
    let resp_bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("failed reading S3 response body: {e}"))?;
    if !status.is_success() {
        let text = String::from_utf8_lossy(&resp_bytes);
        return Err(format!("S3 returned HTTP {status}: {text}"));
    }
    Ok(resp_bytes.to_vec())
}

#[allow(clippy::too_many_arguments)]
fn sign_aws_v4(
    method: &str,
    canonical_uri: &str,
    host: &str,
    region: &str,
    access_key: &str,
    secret_key: &str,
    amz_date: &str,
    date_stamp: &str,
    payload_hash: &str,
) -> Result<String, String> {
    let canonical_headers =
        format!("host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n");
    let signed_headers = "host;x-amz-content-sha256;x-amz-date";
    let canonical_request = format!(
        "{method}\n{canonical_uri}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );
    let scope = format!("{date_stamp}/{region}/s3/aws4_request");
    let canonical_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
    let string_to_sign = format!("AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{canonical_hash}");

    let k_date = hmac_sha256(
        format!("AWS4{secret_key}").as_bytes(),
        date_stamp.as_bytes(),
    )?;
    let k_region = hmac_sha256(&k_date, region.as_bytes())?;
    let k_service = hmac_sha256(&k_region, b"s3")?;
    let k_signing = hmac_sha256(&k_service, b"aws4_request")?;
    let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes())?);

    Ok(format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders={signed_headers}, Signature={signature}"
    ))
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let mut mac =
        HmacSha256::new_from_slice(key).map_err(|_| "invalid HMAC key length".to_string())?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn local_site_backup_and_restore_roundtrip_verifies_sha256() {
        let root = TempDir::new().unwrap();
        let site_dir = root.path().join("site");
        let backup_dir = root.path().join("backups");
        fs::create_dir_all(&site_dir).unwrap();
        fs::create_dir_all(&backup_dir).unwrap();
        fs::write(site_dir.join("index.html"), b"hello-backup-v1").unwrap();

        let create_payload = BackupCreatePayload {
            backup_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            archive_key: "tenant/site.tar.gz".to_string(),
            target: BackupTarget::Site {
                site_id: Uuid::new_v4(),
                domain: "app.local".to_string(),
                document_root: site_dir.to_string_lossy().into_owned(),
            },
            destination: BackupDestination::Local {
                base_dir: backup_dir.to_string_lossy().into_owned(),
            },
        };
        let sha256 = execute_backup_create(&create_payload).await.unwrap();
        assert_eq!(sha256.len(), 64);

        // Delete live file and verify tampered SHA-256 fails before restore
        fs::remove_file(site_dir.join("index.html")).unwrap();
        let bad_restore = BackupRestorePayload {
            backup_id: create_payload.backup_id,
            tenant_id: create_payload.tenant_id,
            archive_key: create_payload.archive_key.clone(),
            target: create_payload.target.clone(),
            destination: create_payload.destination.clone(),
            expected_sha256: Some("0".repeat(64)),
        };
        assert!(execute_backup_restore(&bad_restore).await.is_err());
        assert!(!site_dir.join("index.html").exists());

        // Valid SHA-256 restores exact bytes
        let good_restore = BackupRestorePayload {
            expected_sha256: Some(sha256),
            ..bad_restore
        };
        execute_backup_restore(&good_restore).await.unwrap();
        assert_eq!(
            fs::read(site_dir.join("index.html")).unwrap(),
            b"hello-backup-v1"
        );
    }
}
