#![forbid(unsafe_code)]

#[path = "../src/dump.rs"]
mod dump;

use tokio::fs;

#[tokio::test]
async fn test_stream_dump_success() {
    let output_path = "/tmp/test_dump_success.zst";
    let _ = fs::remove_file(output_path).await;

    // Use "echo" as a mock dump utility
    let result = dump::stream_dump("echo", &["-n", "hello world"], output_path).await;
    assert!(result.is_ok());
    let hash = result.unwrap();

    // Verify chunking and checksum (hash of compressed data)
    let compressed_data = fs::read(output_path).await.unwrap();
    let mut manual_hasher = dump::Sha256::new();
    manual_hasher.update(&compressed_data);
    let expected_hash = manual_hasher.finalize();
    assert_eq!(hash, expected_hash);

    // Verify compression ratio conceptually (though "hello world" is too small to compress well,
    // let's do a compressible test)
}

#[tokio::test]
async fn test_stream_dump_compression_ratio() {
    let output_path = "/tmp/test_dump_compression_ratio.zst";
    let _ = fs::remove_file(output_path).await;

    // Create a dummy script that outputs a lot of repetitive data
    let script_path = "/tmp/dummy_dump.sh";
    fs::write(
        script_path,
        "yes 'this is a very repetitive line' | head -n 10000",
    )
    .await
    .unwrap();

    // Make script executable using chmod
    let _ = std::process::Command::new("chmod")
        .arg("+x")
        .arg(script_path)
        .status();

    let result = dump::stream_dump("sh", &[script_path], output_path).await;
    assert!(result.is_ok());

    let compressed_data = fs::read(output_path).await.unwrap();
    // 10000 lines of ~30 chars each = ~300KB
    // Compressed should be much smaller
    assert!(
        compressed_data.len() < 100_000,
        "Should be highly compressible"
    );
}

#[tokio::test]
#[should_panic(expected = "RESULT=BLOCKED: Database dump utility is missing")]
async fn test_stream_dump_missing_utility() {
    let output_path = "/tmp/test_dump_missing.zst";
    // This should panic
    let _ = dump::stream_dump("non_existent_utility_12345", &[], output_path).await;
}
