#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ArcError {
    #[error("Missing instance number (i=)")]
    MissingInstance,
    #[error("Invalid instance number (i=)")]
    InvalidInstance,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ArcValidationResult {
    Pass,
    Fail,
    InvalidSequence,
}

#[derive(Default)]
struct ArcSet {
    has_aar: bool,
    has_ams: bool,
    has_as: bool,
    cv: Option<String>,
}

pub struct ArcChainValidator;

impl Default for ArcChainValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ArcChainValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate(&self, headers: &[(&str, &str)]) -> Result<ArcValidationResult, ArcError> {
        let mut sets: BTreeMap<u32, ArcSet> = BTreeMap::new();

        for &(name, value) in headers {
            if name.eq_ignore_ascii_case("arc-authentication-results")
                || name.eq_ignore_ascii_case("arc-message-signature")
                || name.eq_ignore_ascii_case("arc-seal")
            {
                let i_str = get_tag_value(value, "i").ok_or(ArcError::MissingInstance)?;
                let i: u32 = i_str.parse().map_err(|_| ArcError::InvalidInstance)?;

                if i == 0 {
                    return Err(ArcError::InvalidInstance);
                }

                let set = sets.entry(i).or_default();

                if name.eq_ignore_ascii_case("arc-authentication-results") {
                    set.has_aar = true;
                } else if name.eq_ignore_ascii_case("arc-message-signature") {
                    set.has_ams = true;
                } else if name.eq_ignore_ascii_case("arc-seal") {
                    set.has_as = true;
                    if let Some(cv) = get_tag_value(value, "cv") {
                        set.cv = Some(cv.to_lowercase());
                    }
                }
            }
        }

        if sets.is_empty() {
            return Ok(ArcValidationResult::Pass);
        }

        let mut expected = 1;
        for (i, set) in &sets {
            if *i != expected {
                return Ok(ArcValidationResult::InvalidSequence);
            }

            if !set.has_aar || !set.has_ams || !set.has_as {
                return Ok(ArcValidationResult::InvalidSequence);
            }

            if let Some(cv) = &set.cv {
                if *i == 1 {
                    if cv != "none" {
                        return Ok(ArcValidationResult::Fail);
                    }
                } else {
                    if cv != "pass" {
                        return Ok(ArcValidationResult::Fail);
                    }
                }
            } else {
                return Ok(ArcValidationResult::InvalidSequence);
            }

            expected += 1;
        }

        Ok(ArcValidationResult::Pass)
    }
}

fn get_tag_value<'a>(value: &'a str, tag: &str) -> Option<&'a str> {
    for part in value.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            if k.trim() == tag {
                return Some(v.trim());
            }
        }
    }
    None
}
