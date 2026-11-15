#![forbid(unsafe_code)]

use crate::error::MailError;
use crate::sieve::{
    MatchType, RelationalMatch, SieveAction, SieveCondition, SieveRule, SieveScript,
};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct SieveVacationConfig {
    pub days: u32,
    pub subject: Option<String>,
    pub addresses: Vec<String>,
    pub message: String,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
}

impl Default for SieveVacationConfig {
    fn default() -> Self {
        Self {
            days: 1,
            subject: None,
            addresses: Vec::new(),
            message: String::new(),
            start_time: None,
            end_time: None,
        }
    }
}

pub struct SieveVacationGenerator;

impl SieveVacationGenerator {
    pub fn generate(config: &SieveVacationConfig) -> Result<String, MailError> {
        let mut script = SieveScript::new();

        // 1. Enforce rate-limiting response periods: days default 1, min 1, max 30
        let days = config.days.clamp(1, 30);

        // 2. :addresses whitelisting required to prevent email loops
        if config.addresses.is_empty() {
            return Err(MailError::InvalidInput(
                "At least one address must be provided for whitelisting to prevent loops."
                    .to_string(),
            ));
        }

        // 3. Filter out automated messages
        let loop_prevention_conditions = SieveCondition::AnyOf(vec![
            SieveCondition::Header {
                header: "Auto-Submitted".to_string(),
                match_type: MatchType::Is,
                value: "auto-replied".to_string(),
            },
            SieveCondition::Header {
                header: "Auto-Submitted".to_string(),
                match_type: MatchType::Is,
                value: "auto-generated".to_string(),
            },
            SieveCondition::Header {
                header: "Precedence".to_string(),
                match_type: MatchType::Regex,
                value: "^(bulk|junk)$".to_string(),
            },
            SieveCondition::Header {
                header: "List-Id".to_string(),
                match_type: MatchType::Matches,
                value: "*".to_string(), // Sieve :matches "*" will match any value
            },
            SieveCondition::Header {
                header: "X-Cron-Env".to_string(),
                match_type: MatchType::Matches,
                value: "*".to_string(),
            },
        ]);

        let mut conditions = vec![SieveCondition::Not(Box::new(loop_prevention_conditions))];

        // 4. Date range activation window
        if let Some(start) = config.start_time {
            conditions.push(SieveCondition::CurrentDate {
                relational: RelationalMatch::GreaterOrEqual,
                zone: "+0000".to_string(),
                value: start.format("%Y-%m-%d").to_string(),
            });
        }

        if let Some(end) = config.end_time {
            conditions.push(SieveCondition::CurrentDate {
                relational: RelationalMatch::LessOrEqual,
                zone: "+0000".to_string(),
                value: end.format("%Y-%m-%d").to_string(),
            });
        }

        script.add_rule(SieveRule {
            name: Some("Auto-Reply".to_string()),
            condition: SieveCondition::AllOf(conditions),
            actions: vec![SieveAction::Vacation {
                days: Some(days),
                subject: config.subject.clone(),
                addresses: config.addresses.clone(),
                message: config.message.clone(),
            }],
        });

        script.generate()
    }
}
