use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WafMode {
    DetectionOnly,
    Prevention,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WafRuleGroup {
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WafConfig {
    pub mode: WafMode,
    pub rule_groups: Vec<WafRuleGroup>,
    pub custom_rules_enabled: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum WafError {
    #[error("No rule groups enabled, WAF would be ineffective")]
    NoRulesEnabled,
    #[error("Rule group names may contain only ASCII letters, digits, '.', '_' and '-'")]
    InvalidRuleGroupName,
}

impl WafConfig {
    pub fn validate(&self) -> Result<(), WafError> {
        let mut has_enabled = false;
        for group in &self.rule_groups {
            if group.name.is_empty()
                || !group
                    .name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
            {
                return Err(WafError::InvalidRuleGroupName);
            }
            if group.enabled {
                has_enabled = true;
            }
        }

        if !has_enabled && !self.custom_rules_enabled {
            return Err(WafError::NoRulesEnabled);
        }

        Ok(())
    }

    pub fn generate_plan(&self) -> Result<String, WafError> {
        self.validate()?;

        let mut plan = String::new();
        plan.push_str(&format!("WAF_MODE={:?}\n", self.mode));
        for group in &self.rule_groups {
            if group.enabled {
                plan.push_str(&format!("INCLUDE_GROUP={}\n", group.name));
            }
        }
        if self.custom_rules_enabled {
            plan.push_str("INCLUDE_CUSTOM_RULES=true\n");
        }

        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_waf_config() {
        let config = WafConfig {
            mode: WafMode::Prevention,
            rule_groups: vec![WafRuleGroup {
                name: "OWASP_CRS".into(),
                enabled: true,
            }],
            custom_rules_enabled: false,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_no_rules_enabled() {
        let config = WafConfig {
            mode: WafMode::Prevention,
            rule_groups: vec![WafRuleGroup {
                name: "OWASP_CRS".into(),
                enabled: false,
            }],
            custom_rules_enabled: false,
        };
        assert!(matches!(config.validate(), Err(WafError::NoRulesEnabled)));
    }

    #[test]
    fn test_invalid_rule_name() {
        let config = WafConfig {
            mode: WafMode::Prevention,
            rule_groups: vec![WafRuleGroup {
                name: "invalid\nname".into(),
                enabled: true,
            }],
            custom_rules_enabled: false,
        };
        assert!(matches!(
            config.validate(),
            Err(WafError::InvalidRuleGroupName)
        ));
    }

    #[test]
    fn rejects_waf_group_directive_injection_and_separators() {
        for name in [
            "OWASP_CRS\nINCLUDE_CUSTOM_RULES=true",
            "OWASP CRS",
            "OWASP_CRS=INCLUDE_CUSTOM_RULES=true",
            "OWASP_CRS;INCLUDE_CUSTOM_RULES=true",
        ] {
            let config = WafConfig {
                mode: WafMode::Prevention,
                rule_groups: vec![WafRuleGroup {
                    name: name.into(),
                    enabled: true,
                }],
                custom_rules_enabled: false,
            };

            assert!(matches!(
                config.validate(),
                Err(WafError::InvalidRuleGroupName)
            ));
            assert!(matches!(
                config.generate_plan(),
                Err(WafError::InvalidRuleGroupName)
            ));
        }
    }
}
