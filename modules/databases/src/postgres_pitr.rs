#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WalFile {
    pub timeline: u32,
    pub log: u32,
    pub seg: u32,
}

impl WalFile {
    pub fn parse(s: &str) -> Result<Self, &'static str> {
        if s.len() != 24 {
            return Err("Invalid WAL filename length, expected 24 characters");
        }
        let timeline = u32::from_str_radix(&s[0..8], 16).map_err(|_| "Invalid timeline hex string")?;
        let log = u32::from_str_radix(&s[8..16], 16).map_err(|_| "Invalid log hex string")?;
        let seg = u32::from_str_radix(&s[16..24], 16).map_err(|_| "Invalid seg hex string")?;
        Ok(Self { timeline, log, seg })
    }

    pub fn to_filename(&self) -> String {
        format!("{:08X}{:08X}{:08X}", self.timeline, self.log, self.seg)
    }

    pub fn absolute_index(&self) -> u64 {
        (self.log as u64) * 256 + (self.seg as u64)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PitrValidationResult {
    pub is_continuous: bool,
    pub missing_segments: Vec<String>,
    pub target_timeline: u32,
}

pub struct PitrValidator;

impl PitrValidator {
    pub fn validate_sequence(files: &[WalFile]) -> PitrValidationResult {
        if files.is_empty() {
            return PitrValidationResult {
                is_continuous: true,
                missing_segments: vec![],
                target_timeline: 0,
            };
        }

        let mut sorted = files.to_vec();
        // Sort by absolute index, then by timeline
        sorted.sort_by_key(|f| (f.absolute_index(), f.timeline));

        let mut missing_segments = Vec::new();
        let mut current_timeline = sorted[0].timeline;
        let mut expected_index = sorted[0].absolute_index();
        let target_timeline = sorted.last().unwrap().timeline;

        let mut prev_file: Option<&WalFile> = None;

        for file in &sorted {
            if let Some(prev) = prev_file {
                let idx = file.absolute_index();
                let prev_idx = prev.absolute_index();

                if idx == prev_idx {
                    // Timeline switch at the same segment
                    current_timeline = file.timeline;
                } else if idx > expected_index {
                    // Gap detected, use the timeline of the previous file
                    for missing_idx in expected_index..idx {
                        let log = (missing_idx / 256) as u32;
                        let seg = (missing_idx % 256) as u32;
                        let missing = WalFile {
                            timeline: current_timeline,
                            log,
                            seg,
                        };
                        missing_segments.push(missing.to_filename());
                    }
                    current_timeline = file.timeline;
                } else {
                    current_timeline = file.timeline;
                }
            } else {
                current_timeline = file.timeline;
            }

            expected_index = file.absolute_index() + 1;
            prev_file = Some(file);
        }

        PitrValidationResult {
            is_continuous: missing_segments.is_empty(),
            missing_segments,
            target_timeline,
        }
    }
}
