use std::io::{Cursor, Write};
use wordpress::install::{InstallError, Orchestrator};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

fn archive(method: CompressionMethod) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            "wordpress/index.php",
            SimpleFileOptions::default().compression_method(method),
        )
        .unwrap();
    writer.write_all(b"hello world").unwrap();
    writer.finish().unwrap().into_inner()
}

async fn install(bytes: Vec<u8>, rejected: bool) {
    let mut server = mockito::Server::new_async().await;
    let checksums = server
        .mock("GET", "/core/checksums/1.0/?version=6.3&locale=en_US")
        .with_header("content-type", "application/json")
        .with_body(r#"{"checksums":{"index.php":"2aae6c35c94fcfb415dbe95f408b9ce91ee846ed"}}"#)
        .create_async()
        .await;
    let download = server
        .mock("GET", "/release/en_US/wordpress-6.3.zip")
        .with_body(bytes)
        .create_async()
        .await;
    let fixture = tempfile::tempdir().unwrap();
    let outside = fixture.path().join("overwrite.php");
    std::fs::write(&outside, b"preserve existing file").unwrap();
    let target = fixture.path().join("install");
    let mut installer = Orchestrator::new(&target);
    installer.api_base_url = server.url();
    installer.download_base_url = server.url();
    let result = installer.download_and_verify("6.3", "en_US").await;
    if rejected {
        assert!(
            matches!(result, Err(InstallError::ZipError(_))),
            "ambiguous ZIP must be rejected, got {result:?}"
        );
        assert_eq!(
            std::fs::read_dir(&target).unwrap().count(),
            0,
            "no promoted files or staging leftovers"
        );
    } else {
        result.unwrap();
        assert_eq!(
            std::fs::read(target.join("index.php")).unwrap(),
            b"hello world"
        );
        assert_eq!(std::fs::read_dir(&target).unwrap().count(), 1);
    }
    assert_eq!(std::fs::read(outside).unwrap(), b"preserve existing file");
    checksums.assert_async().await;
    download.assert_async().await;
}

#[tokio::test]
async fn rejects_local_filename_cloaking() {
    let mut bytes = archive(CompressionMethod::Stored);
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    let name_len = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
    let replacement = b"wordpress/shell.php";
    assert_eq!(name_len, replacement.len());
    bytes[30..30 + name_len].copy_from_slice(replacement);
    install(bytes, true).await;
}

#[tokio::test]
async fn rejects_local_traversal_hidden_by_central_name() {
    let mut bytes = archive(CompressionMethod::Stored);
    let name_len = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
    let replacement = b"../../overwrite.php";
    assert_eq!(name_len, replacement.len());
    bytes[30..30 + name_len].copy_from_slice(replacement);
    install(bytes, true).await;
}

#[tokio::test]
async fn rejects_local_compression_method_mismatch() {
    let mut bytes = archive(CompressionMethod::Stored);
    bytes[8..10].copy_from_slice(&8u16.to_le_bytes()); // Local Deflate, central Stored.
    install(bytes, true).await;
}

#[tokio::test]
async fn accepts_consistent_stored_and_deflated_archives() {
    for method in [CompressionMethod::Stored, CompressionMethod::Deflated] {
        install(archive(method), false).await;
    }
}
