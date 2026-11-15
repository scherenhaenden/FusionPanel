#![forbid(unsafe_code)]

use std::fs;
use std::io;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PsiMetrics {
    pub avg10: f64,
    pub avg60: f64,
    pub avg300: f64,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct IoPsiData {
    pub some: PsiMetrics,
    pub full: PsiMetrics,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IoPressureState {
    Normal,
    Elevated,
    Critical,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IoPressureReport {
    pub data: IoPsiData,
    pub stress_score: f64,
    pub state: IoPressureState,
    pub is_bottleneck: bool,
}

pub struct IoPsiMonitor {
    pub elevated_threshold_avg10: f64,
    pub critical_threshold_avg10: f64,
}

impl Default for IoPsiMonitor {
    fn default() -> Self {
        Self {
            elevated_threshold_avg10: 10.0,
            critical_threshold_avg10: 30.0,
        }
    }
}

impl IoPsiMonitor {
    pub fn new(elevated_threshold: f64, critical_threshold: f64) -> Self {
        Self {
            elevated_threshold_avg10: elevated_threshold,
            critical_threshold_avg10: critical_threshold,
        }
    }

    pub fn parse_psi(content: &str) -> Option<IoPsiData> {
        let mut data = IoPsiData::default();
        let mut found_some = false;
        let mut found_full = false;

        for line in content.lines() {
            let mut parts = line.split_whitespace();
            let prefix = parts.next()?;

            let mut avg10 = 0.0;
            let mut avg60 = 0.0;
            let mut avg300 = 0.0;
            let mut total = 0;

            for part in parts {
                if let Some(eq_idx) = part.find('=') {
                    let key = &part[..eq_idx];
                    let val_str = &part[eq_idx + 1..];
                    match key {
                        "avg10" => avg10 = val_str.parse::<f64>().unwrap_or(0.0),
                        "avg60" => avg60 = val_str.parse::<f64>().unwrap_or(0.0),
                        "avg300" => avg300 = val_str.parse::<f64>().unwrap_or(0.0),
                        "total" => total = val_str.parse::<u64>().unwrap_or(0),
                        _ => {}
                    }
                }
            }

            let metrics = PsiMetrics {
                avg10,
                avg60,
                avg300,
                total,
            };

            if prefix == "some" {
                data.some = metrics;
                found_some = true;
            } else if prefix == "full" {
                data.full = metrics;
                found_full = true;
            }
        }

        if found_some || found_full {
            Some(data)
        } else {
            None
        }
    }

    pub fn read_and_parse(path: &str) -> io::Result<IoPsiData> {
        let content = fs::read_to_string(path)?;
        Self::parse_psi(&content).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid PSI data format"))
    }

    pub fn analyze(&self, data: &IoPsiData) -> IoPressureReport {
        // Stress score could be represented by some avg10
        let stress_score = data.some.avg10;

        let state = if stress_score >= self.critical_threshold_avg10 {
            IoPressureState::Critical
        } else if stress_score >= self.elevated_threshold_avg10 {
            IoPressureState::Elevated
        } else {
            IoPressureState::Normal
        };

        // Bottleneck if "full" stalls are significantly high (e.g., > 10% avg10)
        let is_bottleneck = data.full.avg10 >= 10.0 || state == IoPressureState::Critical;

        IoPressureReport {
            data: data.clone(),
            stress_score,
            state,
            is_bottleneck,
        }
    }
}
