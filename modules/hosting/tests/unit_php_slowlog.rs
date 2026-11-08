#![forbid(unsafe_code)]

use hosting::php_slowlog::{PhpSlowlogParser, SlowlogError};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_parse_valid_slowlog() {
    let log_content = "\n\
[08-Nov-2024 13:09:00]  [pool www] pid 1234\n\
script_filename = /path/to/slow.php\n\
[0x00007f9c8b81d0a0] sleep() /path/to/slow.php:123\n\
[0x00007f9c8b81d0b0] do_work() /path/to/slow.php:45\n\
\n\
[08-Nov-2024 13:10:00]  [pool www] pid 1235\n\
script_filename = /path/to/another.php\n\
[0x00007f9c8b81d0c0] usleep() [internal function]\n\
[0x00007f9c8b81d0d0] some_func() /path/to/another.php:67\n";

    let mut parser = PhpSlowlogParser::new();
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(log_content.as_bytes()).unwrap();

    parser.parse_file(file.path()).unwrap();

    let entries = parser.entries();
    assert_eq!(entries.len(), 2);

    assert_eq!(entries[0].timestamp, "08-Nov-2024 13:09:00");
    assert_eq!(entries[0].pool, "www");
    assert_eq!(entries[0].pid, 1234);
    assert_eq!(entries[0].script_filename, "/path/to/slow.php");
    assert_eq!(entries[0].stack_frames.len(), 2);
    assert_eq!(entries[0].stack_frames[0].function, "sleep");
    assert_eq!(entries[0].stack_frames[0].file, "/path/to/slow.php");
    assert_eq!(entries[0].stack_frames[0].line, 123);

    assert_eq!(entries[1].script_filename, "/path/to/another.php");
    assert_eq!(entries[1].stack_frames[0].function, "usleep");
    assert_eq!(entries[1].stack_frames[0].file, "[internal function]");
    assert_eq!(entries[1].stack_frames[0].line, 0);

    let metrics = parser.metrics();
    assert_eq!(metrics.script_counts.get("/path/to/slow.php"), Some(&1));
    assert_eq!(metrics.script_counts.get("/path/to/another.php"), Some(&1));

    assert_eq!(metrics.function_counts.get("sleep"), Some(&1));
    assert_eq!(metrics.function_counts.get("do_work"), Some(&1));
    assert_eq!(metrics.function_counts.get("usleep"), Some(&1));
    assert_eq!(metrics.function_counts.get("some_func"), Some(&1));
}

#[test]
fn test_path_traversal_detection() {
    let log_content = "\n\
[08-Nov-2024 13:09:00]  [pool www] pid 1234\n\
script_filename = /path/to/../slow.php\n";

    let mut parser = PhpSlowlogParser::new();
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(log_content.as_bytes()).unwrap();

    let result = parser.parse_file(file.path());
    assert!(matches!(result, Err(SlowlogError::PathTraversal)));

    let log_content2 = "\n\
[08-Nov-2024 13:09:00]  [pool www] pid 1234\n\
script_filename = /path/to/slow.php\0\n";
    let mut file2 = NamedTempFile::new().unwrap();
    file2.write_all(log_content2.as_bytes()).unwrap();
    let result2 = parser.parse_file(file2.path());
    assert!(matches!(result2, Err(SlowlogError::PathTraversal)));
}

#[test]
fn test_empty_log() {
    let mut parser = PhpSlowlogParser::new();
    let file = NamedTempFile::new().unwrap();
    parser.parse_file(file.path()).unwrap();
    assert_eq!(parser.entries().len(), 0);
}

#[test]
fn test_malformed_header() {
    let log_content = "\n\
[08-Nov-2024 13:09:00  pool www pid 1234\n";

    let mut parser = PhpSlowlogParser::new();
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(log_content.as_bytes()).unwrap();

    // Since it doesn't match the regex/starts_with exactly, it will either ignore it or fail
    let _ = parser.parse_file(file.path());
}
