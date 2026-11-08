#![forbid(unsafe_code)]

use monitoring::thermal::{FanSensor, ThermalMonitor, ThermalSensor};
use std::fs;

#[test]
fn test_thermal_monitor_parsing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sys_class = temp_dir.path();

    let thermal_dir = sys_class.join("thermal");
    fs::create_dir_all(&thermal_dir).unwrap();

    let zone0 = thermal_dir.join("thermal_zone0");
    fs::create_dir_all(&zone0).unwrap();
    fs::write(zone0.join("temp"), "45000\n").unwrap();

    let hwmon_dir = sys_class.join("hwmon");
    fs::create_dir_all(&hwmon_dir).unwrap();

    let hwmon0 = hwmon_dir.join("hwmon0");
    fs::create_dir_all(&hwmon0).unwrap();
    fs::write(hwmon0.join("temp1_input"), "52000\n").unwrap();
    fs::write(hwmon0.join("fan1_input"), "2400\n").unwrap();

    let monitor = ThermalMonitor::new(sys_class);
    let status = monitor.read_status().unwrap();

    assert_eq!(status.sensors.len(), 2);
    assert!(status.sensors.contains(&ThermalSensor {
        name: "thermal_zone0".to_string(),
        temperature_millicelsius: 45000,
    }));
    assert!(status.sensors.contains(&ThermalSensor {
        name: "hwmon0_temp1_input".to_string(),
        temperature_millicelsius: 52000,
    }));

    assert_eq!(status.fans.len(), 1);
    assert!(status.fans.contains(&FanSensor {
        name: "hwmon0_fan1_input".to_string(),
        rpm: 2400,
    }));
}

#[test]
fn test_thermal_headroom_and_fan_failure() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sys_class = temp_dir.path();

    let hwmon_dir = sys_class.join("hwmon");
    fs::create_dir_all(&hwmon_dir).unwrap();

    let hwmon0 = hwmon_dir.join("hwmon0");
    fs::create_dir_all(&hwmon0).unwrap();

    // Set high temp (86C) which is above warning (85C)
    fs::write(hwmon0.join("temp1_input"), "86000\n").unwrap();
    // Set fan speed to 0 (failure)
    fs::write(hwmon0.join("fan1_input"), "0\n").unwrap();

    let monitor = ThermalMonitor::new(sys_class);
    let status = monitor.read_status().unwrap();

    let headroom = monitor.evaluate_headroom(&status, 85, 95);

    assert_eq!(headroom.max_temperature_millicelsius, 86000);
    assert_eq!(headroom.warning_threshold_millicelsius, 85000);
    assert_eq!(headroom.critical_threshold_millicelsius, 95000);
    assert!(headroom.fan_failure_detected);

    let expected_percent = (86000.0 / 95000.0) * 100.0;
    assert!((headroom.temperature_percent_of_critical - expected_percent).abs() < 0.001);

    let expected_headroom = ((95000.0 - 86000.0) / 95000.0) * 100.0;
    assert!((headroom.headroom_percent - expected_headroom).abs() < 0.001);
}

#[test]
fn test_thermal_fan_working() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sys_class = temp_dir.path();

    let hwmon_dir = sys_class.join("hwmon");
    fs::create_dir_all(&hwmon_dir).unwrap();

    let hwmon0 = hwmon_dir.join("hwmon0");
    fs::create_dir_all(&hwmon0).unwrap();

    // Set high temp (86C) which is above warning (85C)
    fs::write(hwmon0.join("temp1_input"), "86000\n").unwrap();
    // Set fan speed to working value
    fs::write(hwmon0.join("fan1_input"), "3000\n").unwrap();

    let monitor = ThermalMonitor::new(sys_class);
    let status = monitor.read_status().unwrap();

    let headroom = monitor.evaluate_headroom(&status, 85, 95);

    assert_eq!(headroom.max_temperature_millicelsius, 86000);
    assert!(!headroom.fan_failure_detected);
}

#[test]
fn test_no_sensors_fallback() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sys_class = temp_dir.path();

    let monitor = ThermalMonitor::new(sys_class);
    let status = monitor.read_status().unwrap();

    let headroom = monitor.evaluate_headroom(&status, 85, 95);

    assert_eq!(headroom.max_temperature_millicelsius, 0);
    assert_eq!(headroom.temperature_percent_of_critical, 0.0);
    assert_eq!(headroom.headroom_percent, 100.0);
    assert!(!headroom.fan_failure_detected);
}
