#![forbid(unsafe_code)]

use std::collections::BTreeMap;

pub const MIN_PART_SIZE: u64 = 5 * 1024 * 1024; // 5 MiB
pub const MAX_PART_SIZE: u64 = 5 * 1024 * 1024 * 1024; // 5 GiB
pub const MAX_PARTS: u32 = 10_000;
pub const MAX_OBJECT_SIZE: u64 = 5 * 1024 * 1024 * 1024 * 1024; // 5 TiB

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartSpec {
    pub part_number: u32,
    pub offset: u64,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3MultipartPlanner {
    pub total_size: u64,
    pub part_size: u64,
    pub parts: Vec<PartSpec>,
}

impl S3MultipartPlanner {
    pub fn new(total_size: u64) -> Result<Self, &'static str> {
        if total_size > MAX_OBJECT_SIZE {
            return Err("Total size exceeds maximum S3 object size limit (5 TiB)");
        }

        let mut part_size = MIN_PART_SIZE;

        if total_size > 0 {
            let num_parts = (total_size + part_size - 1) / part_size;
            if num_parts > MAX_PARTS as u64 {
                part_size = (total_size + (MAX_PARTS as u64) - 1) / (MAX_PARTS as u64);
            }
            if part_size > MAX_PART_SIZE {
                return Err("Required part size exceeds maximum S3 part size (5 GiB)");
            }
        } else {
            part_size = 0;
        }

        let mut parts = Vec::new();
        let mut offset = 0;
        let mut part_number = 1;

        if total_size == 0 {
            parts.push(PartSpec {
                part_number,
                offset: 0,
                length: 0,
            });
        } else {
            while offset < total_size {
                let remaining = total_size - offset;
                let length = if remaining > part_size {
                    part_size
                } else {
                    remaining
                };

                parts.push(PartSpec {
                    part_number,
                    offset,
                    length,
                });

                offset += length;
                part_number += 1;
            }
        }

        Ok(Self {
            total_size,
            part_size,
            parts,
        })
    }
}

#[derive(Debug, Default)]
pub struct ResumablePartTracker {
    pub completed_parts: BTreeMap<u32, String>,
}

impl ResumablePartTracker {
    pub fn new() -> Self {
        Self {
            completed_parts: BTreeMap::new(),
        }
    }

    pub fn record_part(&mut self, part_number: u32, etag: String) {
        self.completed_parts.insert(part_number, etag);
    }

    pub fn remaining_parts(&self, planner: &S3MultipartPlanner) -> Vec<PartSpec> {
        planner
            .parts
            .iter()
            .filter(|p| !self.completed_parts.contains_key(&p.part_number))
            .cloned()
            .collect()
    }

    pub fn is_complete(&self, planner: &S3MultipartPlanner) -> bool {
        self.remaining_parts(planner).is_empty()
    }

    pub fn verify_manifest(&self, planner: &S3MultipartPlanner) -> Result<Vec<(u32, String)>, &'static str> {
        if !self.is_complete(planner) {
            return Err("Upload incomplete: missing parts");
        }

        let mut manifest = Vec::new();
        for p in &planner.parts {
            if let Some(etag) = self.completed_parts.get(&p.part_number) {
                manifest.push((p.part_number, etag.clone()));
            } else {
                return Err("Upload incomplete: missing parts");
            }
        }

        Ok(manifest)
    }
}
