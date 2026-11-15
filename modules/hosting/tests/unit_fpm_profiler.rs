#![forbid(unsafe_code)]

use hosting::fpm_profiler::{FpmProfiler, FpmProfilerError};

#[test]
fn test_parse_valid_slowlog() {
    let log_data = "\
[08-Nov-2024 13:09:00]  [pool www] pid 1234 (slow execution 1.23 sec)
script_filename = /path/to/script.php
[0x00007f9c8b81d0a0] sleep() /path/to/file.php:123
[0x00007f9c8b81d0a0] my_slow_function() /path/to/script.php:45
[0x00007f9c8b81d0a0] main() [internal function]

[08-Nov-2024 13:10:00]  [pool www] pid 5678 (slow execution 2.5 sec)
script_filename = /path/to/other.php
[0x00007f9c8b81d0b0] curl_exec() [internal function]
[0x00007f9c8b81d0b0] fetch_data() /path/to/other.php:90
";

    let mut profiler = FpmProfiler::new();
    profiler.parse_reader(log_data.as_bytes()).unwrap();

    let entries = profiler.entries();
    assert_eq!(entries.len(), 2);

    let first_entry = &entries[0];
    assert_eq!(first_entry.timestamp, "08-Nov-2024 13:09:00");
    assert_eq!(first_entry.pool, "www");
    assert_eq!(first_entry.pid, 1234);
    assert_eq!(first_entry.duration_sec, Some(1.23));
    assert_eq!(first_entry.script_filename, "/path/to/script.php");
    assert_eq!(first_entry.stack_frames.len(), 3);
    assert_eq!(first_entry.stack_frames[0].function, "sleep");
    assert_eq!(first_entry.stack_frames[0].file, "/path/to/file.php");
    assert_eq!(first_entry.stack_frames[0].line, 123);
    assert_eq!(first_entry.stack_frames[2].function, "main");
    assert_eq!(first_entry.stack_frames[2].file, "[internal function]");
    assert_eq!(first_entry.stack_frames[2].line, 0);

    let second_entry = &entries[1];
    assert_eq!(second_entry.pid, 5678);
    assert_eq!(second_entry.duration_sec, Some(2.5));
    assert_eq!(second_entry.script_filename, "/path/to/other.php");
    assert_eq!(second_entry.stack_frames[0].function, "curl_exec");

    let metrics = profiler.metrics();
    assert_eq!(metrics.script_counts.get("/path/to/script.php"), Some(&1));
    assert_eq!(metrics.script_counts.get("/path/to/other.php"), Some(&1));

    assert_eq!(metrics.function_counts.get("sleep"), Some(&1));
    assert_eq!(metrics.function_counts.get("my_slow_function"), Some(&1));
    assert_eq!(metrics.function_counts.get("main"), Some(&1));
    assert_eq!(metrics.function_counts.get("curl_exec"), Some(&1));

    assert_eq!(metrics.function_durations.get("sleep"), Some(&1.23));
    assert_eq!(metrics.function_durations.get("curl_exec"), Some(&2.5));
}

#[test]
fn test_parse_slowlog_without_duration() {
    let log_data = "\
[08-Nov-2024 13:09:00]  [pool www] pid 1234
script_filename = /path/to/script.php
[0x00007f9c8b81d0a0] sleep() /path/to/file.php:123
";
    let mut profiler = FpmProfiler::new();
    profiler.parse_reader(log_data.as_bytes()).unwrap();

    let entries = profiler.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].duration_sec, None);

    let metrics = profiler.metrics();
    assert!(metrics.function_durations.is_empty());
}

#[test]
fn test_path_traversal_prevention() {
    let log_data = "\
[08-Nov-2024 13:09:00]  [pool www] pid 1234
script_filename = /path/to/../../etc/passwd
[0x00007f9c8b81d0a0] sleep() /path/to/file.php:123
";
    let mut profiler = FpmProfiler::new();
    let err = profiler.parse_reader(log_data.as_bytes()).unwrap_err();
    assert!(matches!(err, FpmProfilerError::PathTraversal));

    let log_data_null = "\
[08-Nov-2024 13:09:00]  [pool www] pid 1234
script_filename = /path/to/file.php\0
[0x00007f9c8b81d0a0] sleep() /path/to/file.php:123
";
    let mut profiler2 = FpmProfiler::new();
    let err2 = profiler2.parse_reader(log_data_null.as_bytes()).unwrap_err();
    assert!(matches!(err2, FpmProfilerError::PathTraversal));
}

#[test]
fn test_malformed_header() {
    // A malformed header would be one that matches `trimmed.starts_with('[') && trimmed.contains("]  [pool ")`
    // but fails `Self::parse_header(trimmed)`. We can make it fail by ensuring "pid " is not in the line,
    // and also making sure "pid " isn't matched incidentally.
    let log_data2 = "\
[08-Nov-2024 13:09:00]  [pool www] somethingelse 1234
";
    let mut profiler = FpmProfiler::new();
    let err = profiler.parse_reader(log_data2.as_bytes()).unwrap_err();
    assert!(matches!(err, FpmProfilerError::MalformedHeader));
}
