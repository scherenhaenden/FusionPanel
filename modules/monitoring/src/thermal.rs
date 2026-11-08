#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct ThermalSensor {
    pub name: String,
    pub temperature_millicelsius: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FanSensor {
    pub name: String,
    pub rpm: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThermalStatus {
    pub sensors: Vec<ThermalSensor>,
    pub fans: Vec<FanSensor>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeadroomStatus {
    pub warning_threshold_millicelsius: i64,
    pub critical_threshold_millicelsius: i64,
    pub max_temperature_millicelsius: i64,
    /// The percentage of the current temperature relative to the critical threshold
    pub temperature_percent_of_critical: f64,
    /// The percentage of remaining headroom before critical threshold is reached
    pub headroom_percent: f64,
    pub fan_failure_detected: bool,
}

pub struct ThermalMonitor {
    sys_class_path: PathBuf,
}

impl ThermalMonitor {
    pub fn new<P: AsRef<Path>>(sys_class_path: P) -> Self {
        Self {
            sys_class_path: sys_class_path.as_ref().to_path_buf(),
        }
    }

    pub fn read_status(&self) -> Result<ThermalStatus, String> {
        let mut sensors = Vec::new();
        let mut fans = Vec::new();

        // Parse thermal_zone
        let thermal_dir = self.sys_class_path.join("thermal");
        if let Ok(entries) = fs::read_dir(&thermal_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("thermal_zone") {
                    let temp_path = entry.path().join("temp");
                    if let Ok(temp_str) = fs::read_to_string(&temp_path) {
                        if let Ok(temp_val) = temp_str.trim().parse::<i64>() {
                            sensors.push(ThermalSensor {
                                name,
                                temperature_millicelsius: temp_val,
                            });
                        }
                    }
                }
            }
        }

        // Parse hwmon
        let hwmon_dir = self.sys_class_path.join("hwmon");
        if let Ok(entries) = fs::read_dir(&hwmon_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("hwmon") {
                    let path = entry.path();
                    if let Ok(hwmon_entries) = fs::read_dir(&path) {
                        for h_entry in hwmon_entries.flatten() {
                            let h_name = h_entry.file_name().to_string_lossy().to_string();

                            // temp*_input
                            if h_name.starts_with("temp") && h_name.ends_with("_input") {
                                if let Ok(val_str) = fs::read_to_string(h_entry.path()) {
                                    if let Ok(val) = val_str.trim().parse::<i64>() {
                                        sensors.push(ThermalSensor {
                                            name: format!("{}_{}", name, h_name),
                                            temperature_millicelsius: val,
                                        });
                                    }
                                }
                            }

                            // fan*_input
                            if h_name.starts_with("fan") && h_name.ends_with("_input") {
                                if let Ok(val_str) = fs::read_to_string(h_entry.path()) {
                                    if let Ok(val) = val_str.trim().parse::<u64>() {
                                        fans.push(FanSensor {
                                            name: format!("{}_{}", name, h_name),
                                            rpm: val,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(ThermalStatus { sensors, fans })
    }

    pub fn evaluate_headroom(
        &self,
        status: &ThermalStatus,
        warning_threshold_celsius: i64,
        critical_threshold_celsius: i64,
    ) -> HeadroomStatus {
        let mut max_temp = i64::MIN;
        let mut has_sensors = false;

        for sensor in &status.sensors {
            has_sensors = true;
            if sensor.temperature_millicelsius > max_temp {
                max_temp = sensor.temperature_millicelsius;
            }
        }

        if !has_sensors {
            max_temp = 0; // fallback if no sensors
        }

        let critical_mc = critical_threshold_celsius * 1000;
        let warning_mc = warning_threshold_celsius * 1000;

        let percent = if critical_mc > 0 {
            (max_temp as f64 / critical_mc as f64) * 100.0
        } else {
            0.0
        };

        let headroom = if critical_mc > 0 {
            ((critical_mc - max_temp) as f64 / critical_mc as f64) * 100.0
        } else {
            0.0
        };

        // Detect cooling fan failure: 0 RPM while high temp (e.g. >= warning threshold)
        let mut fan_failure = false;
        if max_temp >= warning_mc && !status.fans.is_empty() {
            for fan in &status.fans {
                if fan.rpm == 0 {
                    fan_failure = true;
                    break;
                }
            }
        }

        HeadroomStatus {
            warning_threshold_millicelsius: warning_mc,
            critical_threshold_millicelsius: critical_mc,
            max_temperature_millicelsius: max_temp,
            temperature_percent_of_critical: percent,
            headroom_percent: headroom,
            fan_failure_detected: fan_failure,
        }
    }
}
