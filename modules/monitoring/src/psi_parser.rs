#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct PsiMetrics {
    pub avg10: f64,
    pub avg60: f64,
    pub avg300: f64,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PsiData {
    pub some: PsiMetrics,
    pub full: Option<PsiMetrics>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PressureLevel {
    Normal,
    Degraded,
    Critical,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PressureTrend {
    Increasing,
    Decreasing,
    Stable,
}

pub struct PsiParser;

impl PsiParser {
    pub fn parse_file(path: &Path) -> Result<PsiData, String> {
        let content = fs::read_to_string(path).map_err(|e| format!("Failed to read PSI file: {}", e))?;
        Self::parse_content(&content)
    }

    pub fn parse_content(content: &str) -> Result<PsiData, String> {
        let mut some_metrics = None;
        let mut full_metrics = None;

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let kind = parts[0];
            let mut avg10 = None;
            let mut avg60 = None;
            let mut avg300 = None;
            let mut total = None;

            for part in &parts[1..] {
                let kv: Vec<&str> = part.split('=').collect();
                if kv.len() != 2 {
                    continue;
                }
                match kv[0] {
                    "avg10" => avg10 = kv[1].parse::<f64>().ok(),
                    "avg60" => avg60 = kv[1].parse::<f64>().ok(),
                    "avg300" => avg300 = kv[1].parse::<f64>().ok(),
                    "total" => total = kv[1].parse::<u64>().ok(),
                    _ => {}
                }
            }

            let metrics = match (avg10, avg60, avg300, total) {
                (Some(a10), Some(a60), Some(a300), Some(tot)) => PsiMetrics {
                    avg10: a10,
                    avg60: a60,
                    avg300: a300,
                    total: tot,
                },
                _ => continue, // Ignore malformed lines or incomplete metrics
            };

            if kind == "some" {
                some_metrics = Some(metrics);
            } else if kind == "full" {
                full_metrics = Some(metrics);
            }
        }

        let some = some_metrics.ok_or_else(|| "Missing 'some' metrics in PSI data".to_string())?;

        Ok(PsiData {
            some,
            full: full_metrics,
        })
    }

    pub fn calculate_level(metrics: &PsiMetrics, degraded_threshold: f64, critical_threshold: f64) -> PressureLevel {
        if metrics.avg10 >= critical_threshold {
            PressureLevel::Critical
        } else if metrics.avg10 >= degraded_threshold {
            PressureLevel::Degraded
        } else {
            PressureLevel::Normal
        }
    }

    pub fn calculate_trend(metrics: &PsiMetrics) -> PressureTrend {
        // Trend based on comparing short term (avg10) with medium term (avg60)
        let diff = metrics.avg10 - metrics.avg60;

        // Define a small epsilon for stability
        let epsilon = 0.5;

        if diff > epsilon {
            PressureTrend::Increasing
        } else if diff < -epsilon {
            PressureTrend::Decreasing
        } else {
            PressureTrend::Stable
        }
    }
}
