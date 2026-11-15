#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FpmProfilerError {
    #[error("I/O Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Malformed header")]
    MalformedHeader,
    #[error("Path traversal detected")]
    PathTraversal,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StackFrame {
    pub function: String,
    pub file: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlowlogEntry {
    pub timestamp: String,
    pub pool: String,
    pub pid: u32,
    pub duration_sec: Option<f64>,
    pub script_filename: String,
    pub stack_frames: Vec<StackFrame>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct AggregationMetrics {
    pub script_counts: HashMap<String, usize>,
    pub function_counts: HashMap<String, usize>,
    pub function_durations: HashMap<String, f64>,
}

pub struct FpmProfiler {
    metrics: AggregationMetrics,
    entries: Vec<SlowlogEntry>,
}

impl FpmProfiler {
    pub fn new() -> Self {
        Self {
            metrics: AggregationMetrics::default(),
            entries: Vec::new(),
        }
    }

    pub fn parse_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), FpmProfilerError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        self.parse_reader(reader)
    }

    pub fn parse_reader<R: BufRead>(&mut self, reader: R) -> Result<(), FpmProfilerError> {
        let mut lines = reader.lines();

        let mut current_entry: Option<SlowlogEntry> = None;

        while let Some(line_result) = lines.next() {
            let line = line_result?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if let Some(entry) = current_entry.take() {
                    self.add_entry(entry);
                }
                continue;
            }

            // Header: [08-Nov-2024 13:09:00]  [pool www] pid 1234
            // or:     [08-Nov-2024 13:09:00]  [pool www] pid 1234 (slow execution 1.23 sec)
            if trimmed.starts_with('[') && trimmed.contains("]  [pool ") {
                if let Some(entry) = current_entry.take() {
                    self.add_entry(entry);
                }

                if let Some(entry) = Self::parse_header(trimmed) {
                    current_entry = Some(entry);
                } else {
                    return Err(FpmProfilerError::MalformedHeader);
                }
                continue;
            }

            if let Some(ref mut entry) = current_entry {
                // script_filename = /path/to/script.php
                if trimmed.starts_with("script_filename = ") {
                    let path = trimmed.trim_start_matches("script_filename = ").trim();
                    if path.contains("..") || path.contains('\0') {
                        return Err(FpmProfilerError::PathTraversal);
                    }
                    entry.script_filename = path.to_string();
                    continue;
                }

                // [0x00007f9c8b81d0a0] sleep() /path/to/file.php:123
                if trimmed.starts_with('[') && trimmed.contains(']') {
                    if let Some(frame) = Self::parse_frame(trimmed) {
                        entry.stack_frames.push(frame);
                    }
                }
            }
        }

        if let Some(entry) = current_entry {
            self.add_entry(entry);
        }

        Ok(())
    }

    fn add_entry(&mut self, entry: SlowlogEntry) {
        if !entry.script_filename.is_empty() {
            *self.metrics.script_counts.entry(entry.script_filename.clone()).or_insert(0) += 1;
        }

        // For function counts, we just increment for each distinct function in the stack trace,
        // or for the top blocking function. We'll count every function present in the stack trace.
        for frame in &entry.stack_frames {
            *self.metrics.function_counts.entry(frame.function.clone()).or_insert(0) += 1;
        }

        // If we have a duration, we might want to attribute it to the topmost blocking function (the first one)
        if let Some(duration) = entry.duration_sec {
            if let Some(top_frame) = entry.stack_frames.first() {
                *self.metrics.function_durations.entry(top_frame.function.clone()).or_insert(0.0) += duration;
            }
        }

        self.entries.push(entry);
    }

    fn parse_header(line: &str) -> Option<SlowlogEntry> {
        let date_end = line.find(']')?;
        let timestamp = line[1..date_end].to_string();

        let pool_start = line.find("[pool ")? + 6;
        let pool_end = line[pool_start..].find(']')? + pool_start;
        let pool = line[pool_start..pool_end].to_string();

        let pid_start = line.find("pid ")? + 4;
        let pid_str = &line[pid_start..];

        let space_idx = pid_str.find(' ');
        let (pid_val_str, rest) = if let Some(idx) = space_idx {
            (pid_str[..idx].trim(), pid_str[idx..].trim())
        } else {
            (pid_str, "")
        };

        let pid = pid_val_str.parse::<u32>().ok()?;

        let mut duration_sec = None;
        if rest.starts_with("(slow execution ") {
            let duration_start = "(slow execution ".len();
            if let Some(sec_idx) = rest.find(" sec)") {
                if let Ok(dur) = rest[duration_start..sec_idx].parse::<f64>() {
                    duration_sec = Some(dur);
                }
            }
        }

        Some(SlowlogEntry {
            timestamp,
            pool,
            pid,
            duration_sec,
            script_filename: String::new(),
            stack_frames: Vec::new(),
        })
    }

    fn parse_frame(line: &str) -> Option<StackFrame> {
        let close_bracket = line.find(']')?;
        let rest = line[close_bracket + 1..].trim();

        let func_end = rest.find("()")?;
        let function = rest[..func_end].to_string();

        let file_line_part = rest[func_end + 2..].trim();

        if file_line_part == "[internal function]" {
            return Some(StackFrame {
                function,
                file: "[internal function]".to_string(),
                line: 0,
            });
        }

        let last_colon = file_line_part.rfind(':');
        if let Some(colon_pos) = last_colon {
            let file = file_line_part[..colon_pos].to_string();
            let line_str = &file_line_part[colon_pos + 1..];
            let line_num = line_str.parse::<usize>().unwrap_or(0);
            return Some(StackFrame {
                function,
                file,
                line: line_num,
            });
        }

        None
    }

    pub fn metrics(&self) -> &AggregationMetrics {
        &self.metrics
    }

    pub fn entries(&self) -> &[SlowlogEntry] {
        &self.entries
    }
}
