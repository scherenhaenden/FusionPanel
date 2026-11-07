#![forbid(unsafe_code)]

use crate::error::MailError;
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchType {
    Is,
    Contains,
    Matches,
    Regex,
}

impl MatchType {
    fn as_str(&self) -> &'static str {
        match self {
            MatchType::Is => ":is",
            MatchType::Contains => ":contains",
            MatchType::Matches => ":matches",
            MatchType::Regex => ":regex",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SieveCondition {
    Header {
        header: String,
        match_type: MatchType,
        value: String,
    },
    SizeOver(u64),
    SizeUnder(u64),
    True,
    AnyOf(Vec<SieveCondition>),
    AllOf(Vec<SieveCondition>),
    Not(Box<SieveCondition>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SieveAction {
    FileInto(String),
    Redirect(String),
    RedirectCopy(String),
    Keep,
    Discard,
    Reject(String),
    Vacation {
        days: Option<u32>,
        subject: Option<String>,
        addresses: Vec<String>,
        message: String,
    },
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SieveRule {
    pub name: Option<String>,
    pub condition: SieveCondition,
    pub actions: Vec<SieveAction>,
}

#[derive(Debug, Clone, Default)]
pub struct SieveScript {
    pub rules: Vec<SieveRule>,
}

impl SieveScript {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_rule(&mut self, rule: SieveRule) {
        self.rules.push(rule);
    }

    fn collect_requires(&self) -> Vec<&'static str> {
        let mut requires = Vec::new();
        for rule in &self.rules {
            for action in &rule.actions {
                match action {
                    SieveAction::FileInto(_) => {
                        if !requires.contains(&"fileinto") {
                            requires.push("fileinto");
                        }
                    }
                    SieveAction::Vacation { .. } => {
                        if !requires.contains(&"vacation") {
                            requires.push("vacation");
                        }
                    }
                    SieveAction::Reject(_) => {
                        if !requires.contains(&"reject") {
                            requires.push("reject");
                        }
                    }
                    SieveAction::RedirectCopy(_) => {
                        if !requires.contains(&"copy") {
                            requires.push("copy");
                        }
                    }
                    _ => {}
                }
            }
            if Self::condition_requires_regex(&rule.condition) && !requires.contains(&"regex") {
                requires.push("regex");
            }
        }
        requires
    }

    fn condition_requires_regex(cond: &SieveCondition) -> bool {
        match cond {
            SieveCondition::Header {
                match_type: MatchType::Regex,
                ..
            } => true,
            SieveCondition::AnyOf(conds) | SieveCondition::AllOf(conds) => {
                conds.iter().any(Self::condition_requires_regex)
            }
            SieveCondition::Not(c) => Self::condition_requires_regex(c),
            _ => false,
        }
    }

    fn escape_string(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }

    fn format_condition(cond: &SieveCondition) -> String {
        match cond {
            SieveCondition::Header {
                header,
                match_type,
                value,
            } => {
                format!(
                    "header {} \"{}\" \"{}\"",
                    match_type.as_str(),
                    Self::escape_string(header),
                    Self::escape_string(value)
                )
            }
            SieveCondition::SizeOver(size) => format!("size :over {}", size),
            SieveCondition::SizeUnder(size) => format!("size :under {}", size),
            SieveCondition::True => "true".to_string(),
            SieveCondition::AnyOf(conds) => {
                if conds.is_empty() {
                    "false".to_string()
                } else {
                    let formatted: Vec<String> = conds.iter().map(Self::format_condition).collect();
                    format!("anyof ({})", formatted.join(", "))
                }
            }
            SieveCondition::AllOf(conds) => {
                if conds.is_empty() {
                    "true".to_string()
                } else {
                    let formatted: Vec<String> = conds.iter().map(Self::format_condition).collect();
                    format!("allof ({})", formatted.join(", "))
                }
            }
            SieveCondition::Not(c) => format!("not {}", Self::format_condition(c)),
        }
    }

    fn format_action(action: &SieveAction) -> String {
        match action {
            SieveAction::FileInto(folder) => {
                format!("fileinto \"{}\";", Self::escape_string(folder))
            }
            SieveAction::Redirect(address) => {
                format!("redirect \"{}\";", Self::escape_string(address))
            }
            SieveAction::RedirectCopy(address) => {
                format!("redirect :copy \"{}\";", Self::escape_string(address))
            }
            SieveAction::Keep => "keep;".to_string(),
            SieveAction::Discard => "discard;".to_string(),
            SieveAction::Stop => "stop;".to_string(),
            SieveAction::Reject(reason) => format!("reject \"{}\";", Self::escape_string(reason)),
            SieveAction::Vacation {
                days,
                subject,
                addresses,
                message,
            } => {
                let mut parts = vec!["vacation".to_string()];
                if let Some(d) = days {
                    parts.push(format!(":days {}", d));
                }
                if let Some(s) = subject {
                    parts.push(format!(":subject \"{}\"", Self::escape_string(s)));
                }
                if !addresses.is_empty() {
                    let addrs: Vec<String> = addresses
                        .iter()
                        .map(|a| format!("\"{}\"", Self::escape_string(a)))
                        .collect();
                    parts.push(format!(":addresses [{}]", addrs.join(", ")));
                }

                if message.contains('\n') {
                    let stuffed_message = message
                        .lines()
                        .map(|line| {
                            if line.starts_with('.') {
                                format!(".{}", line)
                            } else {
                                line.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    parts.push(format!("text:\n{}\n.\n;", stuffed_message));
                    parts.join(" ")
                } else {
                    parts.push(format!("\"{}\"", Self::escape_string(message)));
                    format!("{};", parts.join(" "))
                }
            }
        }
    }

    pub fn generate(&self) -> Result<String, MailError> {
        let mut script = String::new();

        let requires = self.collect_requires();
        if !requires.is_empty() {
            let reqs: Vec<String> = requires.iter().map(|r| format!("\"{}\"", r)).collect();
            writeln!(&mut script, "require [{}];", reqs.join(", "))
                .map_err(|e| MailError::OperationFailed(e.to_string()))?;
            writeln!(&mut script).map_err(|e| MailError::OperationFailed(e.to_string()))?;
        }

        for (i, rule) in self.rules.iter().enumerate() {
            if let Some(name) = &rule.name {
                writeln!(&mut script, "# Rule: {}", name)
                    .map_err(|e| MailError::OperationFailed(e.to_string()))?;
            }

            let condition_str = Self::format_condition(&rule.condition);
            writeln!(&mut script, "if {} {{", condition_str)
                .map_err(|e| MailError::OperationFailed(e.to_string()))?;

            for action in &rule.actions {
                let action_str = Self::format_action(action);
                if action_str.contains('\n') {
                    for (line_idx, line) in action_str.lines().enumerate() {
                        if line_idx == 0 {
                            writeln!(&mut script, "    {}", line)
                                .map_err(|e| MailError::OperationFailed(e.to_string()))?;
                        } else {
                            writeln!(&mut script, "{}", line)
                                .map_err(|e| MailError::OperationFailed(e.to_string()))?;
                        }
                    }
                } else {
                    writeln!(&mut script, "    {}", action_str)
                        .map_err(|e| MailError::OperationFailed(e.to_string()))?;
                }
            }
            writeln!(&mut script, "}}").map_err(|e| MailError::OperationFailed(e.to_string()))?;

            if i < self.rules.len() - 1 {
                writeln!(&mut script).map_err(|e| MailError::OperationFailed(e.to_string()))?;
            }
        }

        Ok(script)
    }
}
