#![forbid(unsafe_code)]

use monitoring::swap_thrashing::{SwapThrashingDetector, ThrashingLevel};
use std::fs;
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;

#[test]
fn test_healthy_system() {
    let vmstat_file = NamedTempFile::new().unwrap();
    let meminfo_file = NamedTempFile::new().unwrap();

    fs::write(vmstat_file.path(), "pswpin 100\npswpout 200\n").unwrap();
    fs::write(
        meminfo_file.path(),
        "SwapTotal: 1048576 kB\nSwapFree: 524288 kB\nMemAvailable: 2097152 kB\n",
    ).unwrap();

    let detector = SwapThrashingDetector::new(
        vmstat_file.path().to_str().unwrap(),
        meminfo_file.path().to_str().unwrap(),
    );

    let start_time = Instant::now();
    let report1 = detector.analyze_at(start_time).unwrap();
    
    assert_eq!(report1.level, ThrashingLevel::Healthy);
    assert_eq!(report1.swap_in_rate, 0.0);
    assert_eq!(report1.swap_out_rate, 0.0);
    assert_eq!(report1.swap_total, 1048576 * 1024);
    assert_eq!(report1.swap_free, 524288 * 1024);
    assert_eq!(report1.mem_available, 2097152 * 1024);

    // Second analysis
    let time2 = start_time + Duration::from_secs(1);
    fs::write(vmstat_file.path(), "pswpin 110\npswpout 220\n").unwrap();
    let report2 = detector.analyze_at(time2).unwrap();
    
    assert_eq!(report2.level, ThrashingLevel::Healthy);
    assert_eq!(report2.swap_in_rate, 10.0);
    assert_eq!(report2.swap_out_rate, 20.0);
}

#[test]
fn test_moderate_pressure() {
    let vmstat_file = NamedTempFile::new().unwrap();
    let meminfo_file = NamedTempFile::new().unwrap();

    fs::write(vmstat_file.path(), "pswpin 100\npswpout 200\n").unwrap();
    fs::write(
        meminfo_file.path(),
        "SwapTotal: 1048576 kB\nSwapFree: 524288 kB\nMemAvailable: 2097152 kB\n",
    ).unwrap();

    let detector = SwapThrashingDetector::new(
        vmstat_file.path().to_str().unwrap(),
        meminfo_file.path().to_str().unwrap(),
    );

    let start_time = Instant::now();
    detector.analyze_at(start_time).unwrap();

    let time2 = start_time + Duration::from_secs(1);
    // Increase is 150 (in) + 150 (out) = 300 > 100
    fs::write(vmstat_file.path(), "pswpin 250\npswpout 350\n").unwrap();
    let report2 = detector.analyze_at(time2).unwrap();
    
    assert_eq!(report2.level, ThrashingLevel::ModeratePressure);
}

#[test]
fn test_severe_thrashing() {
    let vmstat_file = NamedTempFile::new().unwrap();
    let meminfo_file = NamedTempFile::new().unwrap();

    fs::write(vmstat_file.path(), "pswpin 100\npswpout 200\n").unwrap();
    fs::write(
        meminfo_file.path(),
        "SwapTotal: 1048576 kB\nSwapFree: 524288 kB\nMemAvailable: 2097152 kB\n",
    ).unwrap();

    let detector = SwapThrashingDetector::new(
        vmstat_file.path().to_str().unwrap(),
        meminfo_file.path().to_str().unwrap(),
    );

    let start_time = Instant::now();
    detector.analyze_at(start_time).unwrap();

    let time2 = start_time + Duration::from_secs(1);
    // Increase is 600 (in) + 600 (out) = 1200 > 1000
    fs::write(vmstat_file.path(), "pswpin 700\npswpout 800\n").unwrap();
    let report2 = detector.analyze_at(time2).unwrap();
    
    assert_eq!(report2.level, ThrashingLevel::SevereThrashing);
}

#[test]
fn test_missing_files() {
    let detector = SwapThrashingDetector::new("/nonexistent/vmstat", "/nonexistent/meminfo");
    let result = detector.analyze();
    assert!(result.is_err());
}

#[test]
fn test_missing_keys_vmstat() {
    let vmstat_file = NamedTempFile::new().unwrap();
    let meminfo_file = NamedTempFile::new().unwrap();

    fs::write(vmstat_file.path(), "otherkey 100\n").unwrap();
    fs::write(
        meminfo_file.path(),
        "SwapTotal: 1048576 kB\nSwapFree: 524288 kB\nMemAvailable: 2097152 kB\n",
    ).unwrap();

    let detector = SwapThrashingDetector::new(
        vmstat_file.path().to_str().unwrap(),
        meminfo_file.path().to_str().unwrap(),
    );

    let result = detector.analyze();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Failed to parse required keys from vmstat");
}

#[test]
fn test_counter_rollover() {
    let vmstat_file = NamedTempFile::new().unwrap();
    let meminfo_file = NamedTempFile::new().unwrap();

    // High initial values
    fs::write(vmstat_file.path(), "pswpin 18446744073709551600\npswpout 18446744073709551600\n").unwrap();
    fs::write(
        meminfo_file.path(),
        "SwapTotal: 1048576 kB\nSwapFree: 524288 kB\nMemAvailable: 2097152 kB\n",
    ).unwrap();

    let detector = SwapThrashingDetector::new(
        vmstat_file.path().to_str().unwrap(),
        meminfo_file.path().to_str().unwrap(),
    );

    let start_time = Instant::now();
    detector.analyze_at(start_time).unwrap();

    // Simulating rollover: values reset or are lower than previous
    let time2 = start_time + Duration::from_secs(1);
    fs::write(vmstat_file.path(), "pswpin 10\npswpout 10\n").unwrap();
    
    let report = detector.analyze_at(time2).unwrap();
    // In case of rollover, rate calculation should ignore negative change and remain at 0.0 (or we could calculate wrapping, but ignoring is fine to avoid panics as implemented)
    assert_eq!(report.swap_in_rate, 0.0);
    assert_eq!(report.swap_out_rate, 0.0);
    assert_eq!(report.level, ThrashingLevel::Healthy);
}

#[test]
fn test_recommendations() {
    let vmstat_file = NamedTempFile::new().unwrap();
    let meminfo_file = NamedTempFile::new().unwrap();

    fs::write(vmstat_file.path(), "pswpin 100\npswpout 200\n").unwrap();
    // Low mem available (< 100MB), low swap free (< 10%)
    fs::write(
        meminfo_file.path(),
        "SwapTotal: 1048576 kB\nSwapFree: 50000 kB\nMemAvailable: 50000 kB\n",
    ).unwrap();

    let detector = SwapThrashingDetector::new(
        vmstat_file.path().to_str().unwrap(),
        meminfo_file.path().to_str().unwrap(),
    );

    let report = detector.analyze().unwrap();
    
    assert!(report.recommendations.iter().any(|r| r.contains("Available memory is critically low")));
    assert!(report.recommendations.iter().any(|r| r.contains("Swap space is running out")));
}
