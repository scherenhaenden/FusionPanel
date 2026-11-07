#![forbid(unsafe_code)]

#[path = "../src/dkim.rs"]
mod dkim;

use dkim::{format_dns_txt_record, generate_dkim_keypair};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

#[test]
fn test_dkim_keypair_generation() {
    let dir = tempdir().expect("Failed to create temp dir");
    let key_path = dir.path().join("test_key.pem");

    let result = generate_dkim_keypair(&key_path).expect("Failed to generate DKIM keypair");

    // Check private key
    assert!(result.private_key.contains("BEGIN PRIVATE KEY"));
    assert!(result.private_key.contains("END PRIVATE KEY"));

    // Check public key
    assert!(result.public_key.contains("BEGIN PUBLIC KEY"));
    assert!(result.public_key.contains("END PUBLIC KEY"));

    // Check key length (2048-bit)
    assert!(
        result.private_key.len() > 1600 && result.private_key.len() < 1800,
        "RSA 2048-bit private key length should be around 1670-1704 characters"
    );
    assert!(
        result.public_key.len() > 400 && result.public_key.len() < 500,
        "RSA 2048-bit public key length should be around 450 characters"
    );

    // Check base64 encoding (alphanumeric + +/ + =)
    let stripped = result
        .public_key
        .replace("-----BEGIN PUBLIC KEY-----", "")
        .replace("-----END PUBLIC KEY-----", "")
        .replace("\n", "")
        .replace("\r", "");
    for c in stripped.chars() {
        assert!(
            c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=',
            "Invalid character in base64 string: {}",
            c
        );
    }

    // Check file exists
    assert!(key_path.exists());

    // Check file permissions (0600)
    let metadata = fs::metadata(&key_path).expect("Failed to get file metadata");
    let mode = metadata.permissions().mode();
    // In unix, the mode includes the file type, so we mask it to get just the permissions
    assert_eq!(mode & 0o777, 0o600, "File permissions are not 0600");
}

#[test]
fn test_format_dns_txt_record() {
    let public_key = "-----BEGIN PUBLIC KEY-----\n\
        MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA1ppS3UZcs5JOdStc3s8M\n\
        Pg0n/5nCwbFmaJyCyxlP81GhLXBY6qk6gGJk4E+S8Xq1NYlzoIFjCTZ4yhXrE5H+\n\
        fgIXFZt/w59YuZ9SA1iZpHoOa2L4O/rTExFuWazkBKHE6rnNHhrXQWBOkCBtGobR\n\
        o6DWnV2sZ4qNPyNRqbH2IYefUJTpDMTpsFO+ziQ0U+ww2Rdkr7dB9oOPzwCy2aJZ\n\
        Cm0vJYsrKf2ESnSM0BFZ9//xlsxRvw1BrqqQ/53b4bjZzGsuacsAvbk1xdzObxfd\n\
        McgMMGwmP8B2bYiohmRf+/mJDACrdtWsjGaiqjhLvSi29QdJTcPFFs3sC0X1dKSI\n\
        GwIDAQAB\n\
        -----END PUBLIC KEY-----\n";

    let txt_record = format_dns_txt_record(public_key).expect("Failed to format txt record");

    let expected_base64 = "\
        MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA1ppS3UZcs5JOdStc3s8M\
        Pg0n/5nCwbFmaJyCyxlP81GhLXBY6qk6gGJk4E+S8Xq1NYlzoIFjCTZ4yhXrE5H+\
        fgIXFZt/w59YuZ9SA1iZpHoOa2L4O/rTExFuWazkBKHE6rnNHhrXQWBOkCBtGobR\
        o6DWnV2sZ4qNPyNRqbH2IYefUJTpDMTpsFO+ziQ0U+ww2Rdkr7dB9oOPzwCy2aJZ\
        Cm0vJYsrKf2ESnSM0BFZ9//xlsxRvw1BrqqQ/53b4bjZzGsuacsAvbk1xdzObxfd\
        McgMMGwmP8B2bYiohmRf+/mJDACrdtWsjGaiqjhLvSi29QdJTcPFFs3sC0X1dKSI\
        GwIDAQAB";

    let expected_str = format!("v=DKIM1; k=rsa; p={}", expected_base64);

    // Verify chunking logic
    assert!(txt_record.contains("\"v=DKIM1; k=rsa; p="));

    // Split by the space between quotes: '" "'
    let chunks: Vec<&str> = txt_record.split("\" \"").collect();
    assert_eq!(chunks.len(), 2, "Expected 2 chunks");

    for (i, chunk) in chunks.iter().enumerate() {
        let is_first = i == 0;
        let is_last = i == chunks.len() - 1;

        if is_first {
            assert!(chunk.starts_with('"'));
        }
        if is_last {
            assert!(chunk.ends_with('"'));
        }

        let mut len = chunk.len();
        if is_first {
            len -= 1;
        }
        if is_last {
            len -= 1;
        }
        assert!(len <= 255);
    }

    let recombined = txt_record.replace("\" \"", "").replace("\"", "");
    assert_eq!(recombined, expected_str);
}
