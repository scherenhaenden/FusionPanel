#![forbid(unsafe_code)]


pub const WRAPAROUND_THRESHOLD_MAX: u32 = 2_147_483_648; // 2^31
pub const DEFAULT_AUTOVACUUM_FREEZE_MAX_AGE: u32 = 200_000_000;
pub const DEFAULT_WRAPAROUND_THRESHOLD: u32 = 2_000_000_000;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FreezeGuardLevel {
    Normal,
    WarningAutovacuum,
    CriticalAutovacuum,
    WarningWraparound,
    EmergencyWraparound,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FreezeGuardConfig {
    pub autovacuum_freeze_max_age: u32,
    pub wraparound_threshold: u32,
    pub autovacuum_warning_threshold: u32,
    pub wraparound_warning_threshold: u32,
}

impl Default for FreezeGuardConfig {
    fn default() -> Self {
        Self {
            autovacuum_freeze_max_age: DEFAULT_AUTOVACUUM_FREEZE_MAX_AGE,
            wraparound_threshold: DEFAULT_WRAPAROUND_THRESHOLD,
            autovacuum_warning_threshold: DEFAULT_AUTOVACUUM_FREEZE_MAX_AGE / 10 * 8,
            wraparound_warning_threshold: DEFAULT_WRAPAROUND_THRESHOLD / 10 * 9,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FreezeGuardStatus {
    pub name: String,
    pub age: u32,
    pub remaining_to_autovacuum: u32,
    pub remaining_to_wraparound: u32,
    pub level: FreezeGuardLevel,
}

pub fn calculate_xid_age(current_xid: u32, frozen_xid: u32) -> u32 {
    current_xid.wrapping_sub(frozen_xid)
}

pub fn parse_frozen_xid(xid_str: &str) -> Result<u32, std::num::ParseIntError> {
    xid_str.trim().parse::<u32>()
}

pub fn evaluate_freeze_status(
    name: &str,
    current_xid: u32,
    frozen_xid: u32,
    config: &FreezeGuardConfig,
) -> FreezeGuardStatus {
    let age = calculate_xid_age(current_xid, frozen_xid);

    let remaining_to_autovacuum = config.autovacuum_freeze_max_age.saturating_sub(age);
    let remaining_to_wraparound = config.wraparound_threshold.saturating_sub(age);

    let level = if age >= config.wraparound_threshold {
        FreezeGuardLevel::EmergencyWraparound
    } else if age >= config.wraparound_warning_threshold {
        FreezeGuardLevel::WarningWraparound
    } else if age >= config.autovacuum_freeze_max_age {
        FreezeGuardLevel::CriticalAutovacuum
    } else if age >= config.autovacuum_warning_threshold {
        FreezeGuardLevel::WarningAutovacuum
    } else {
        FreezeGuardLevel::Normal
    };

    FreezeGuardStatus {
        name: name.to_string(),
        age,
        remaining_to_autovacuum,
        remaining_to_wraparound,
        level,
    }
}
