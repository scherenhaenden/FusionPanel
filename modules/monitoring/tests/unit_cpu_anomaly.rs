#![forbid(unsafe_code)]

use monitoring::cpu_anomaly::{CpuAnomalyDetector, Anomaly};

#[test]
fn test_normal_behavior() {
    let mut detector = CpuAnomalyDetector::new(4);

    let stat1 = "cpu  100 0 50 1000 10 0 0 0\n";
    let load1 = "1.0 1.0 1.0 1/100 12345";

    let anomalies1 = detector.analyze(stat1, load1);
    assert!(anomalies1.is_empty());

    let stat2 = "cpu  150 0 75 1500 15 0 0 0\n";
    let load2 = "1.5 1.2 1.0 1/100 12345";

    let anomalies2 = detector.analyze(stat2, load2);
    assert!(anomalies2.is_empty());
}

#[test]
fn test_high_iowait() {
    let mut detector = CpuAnomalyDetector::new(4);

    // total diff: 100, iowait diff: 40 (40%)
    let stat1 = "cpu  100 0 50 1000 10 0 0 0\n";
    let load1 = "1.0 1.0 1.0 1/100 12345";
    detector.analyze(stat1, load1);

    let stat2 = "cpu  110 0 60 1040 50 0 0 0\n";
    let load2 = "1.0 1.0 1.0 1/100 12345";
    let anomalies = detector.analyze(stat2, load2);

    assert_eq!(anomalies.len(), 1);
    match anomalies[0] {
        Anomaly::HighIoWait(val) => {
            assert!((val - 40.0).abs() < f64::EPSILON);
        },
        _ => panic!("Expected HighIoWait anomaly"),
    }
}

#[test]
fn test_high_steal_time() {
    let mut detector = CpuAnomalyDetector::new(4);

    // total diff: 100, steal diff: 20 (20%)
    let stat1 = "cpu  100 0 50 1000 10 0 0 0\n";
    let load1 = "1.0 1.0 1.0 1/100 12345";
    detector.analyze(stat1, load1);

    let stat2 = "cpu  120 0 70 1040 10 0 0 20\n";
    let load2 = "1.0 1.0 1.0 1/100 12345";
    let anomalies = detector.analyze(stat2, load2);

    assert_eq!(anomalies.len(), 1);
    match anomalies[0] {
        Anomaly::HighStealTime(val) => {
            assert!((val - 20.0).abs() < f64::EPSILON);
        },
        _ => panic!("Expected HighStealTime anomaly"),
    }
}

#[test]
fn test_runaway_spike() {
    // 4 logical cores, saturation > 1.5 means 1 min load > 6.0
    let mut detector = CpuAnomalyDetector::new(4);

    let stat1 = "cpu  100 0 50 1000 10 0 0 0\n";
    let load1 = "1.0 1.0 1.0 1/100 12345";
    detector.analyze(stat1, load1);

    let stat2 = "cpu  150 0 100 1000 10 0 0 0\n";
    // 1m load = 7.0, 5m load = 2.0. Saturation = 7.0/4 = 1.75
    let load2 = "7.0 2.0 1.0 1/100 12345";
    let anomalies = detector.analyze(stat2, load2);

    assert_eq!(anomalies.len(), 1);
    match anomalies[0] {
        Anomaly::RunawaySpike(val) => {
            assert!((val - 1.75).abs() < f64::EPSILON);
        },
        _ => panic!("Expected RunawaySpike anomaly"),
    }
}

#[test]
fn test_multiple_anomalies() {
    let mut detector = CpuAnomalyDetector::new(4);

    // total diff: 100, iowait diff: 35 (35%), steal diff: 20 (20%)
    let stat1 = "cpu  100 0 50 1000 10 0 0 0\n";
    let load1 = "1.0 1.0 1.0 1/100 12345";
    detector.analyze(stat1, load1);

    let stat2 = "cpu  110 0 60 1025 45 0 0 20\n";
    // 1m load = 6.5, 5m load = 4.0. Saturation = 6.5/4 = 1.625
    let load2 = "6.5 4.0 1.0 1/100 12345";
    let anomalies = detector.analyze(stat2, load2);

    assert_eq!(anomalies.len(), 3);

    let mut found_iowait = false;
    let mut found_steal = false;
    let mut found_spike = false;

    for anomaly in anomalies {
        match anomaly {
            Anomaly::HighIoWait(val) => {
                assert!((val - 35.0).abs() < f64::EPSILON);
                found_iowait = true;
            },
            Anomaly::HighStealTime(val) => {
                assert!((val - 20.0).abs() < f64::EPSILON);
                found_steal = true;
            },
            Anomaly::RunawaySpike(val) => {
                assert!((val - 1.625).abs() < f64::EPSILON);
                found_spike = true;
            },
        }
    }

    assert!(found_iowait);
    assert!(found_steal);
    assert!(found_spike);
}
