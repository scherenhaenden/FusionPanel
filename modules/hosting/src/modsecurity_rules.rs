#![forbid(unsafe_code)]

use std::fmt::Write;

/// Preset applications for OWASP CRS exclusions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Preset {
    WordPress,
    Nextcloud,
}

impl Preset {
    pub fn tx_variable(&self) -> &'static str {
        match self {
            Preset::WordPress => "setvar:tx.crs_exclusions_wordpress=1",
            Preset::Nextcloud => "setvar:tx.crs_exclusions_nextcloud=1",
        }
    }
}

/// Custom URI rule action types.
#[derive(Debug, Clone)]
pub enum CustomAction {
    /// ctl:ruleRemoveById
    RemoveById(u32),
    /// ctl:ruleRemoveTargetById
    RemoveTargetById(u32, String),
}

/// A custom rule targeting a specific URI.
#[derive(Debug, Clone)]
pub struct CustomUriRule {
    pub uri_match: String,
    pub rule_id: u32,
    pub action: CustomAction,
}

/// Builder for generating Nginx ModSecurity / OWASP CRS exclusion rule blocks.
#[derive(Debug, Clone, Default)]
pub struct ModSecurityExclusionsBuilder {
    presets: Vec<Preset>,
    global_removals: Vec<u32>,
    custom_uri_rules: Vec<CustomUriRule>,
    preset_rule_id: u32,
}

impl ModSecurityExclusionsBuilder {
    /// Creates a new builder with the default preset rule ID set to 10000.
    pub fn new() -> Self {
        Self {
            presets: Vec::new(),
            global_removals: Vec::new(),
            custom_uri_rules: Vec::new(),
            preset_rule_id: 10000,
        }
    }

    /// Sets the rule ID used for the SecAction block that enables presets.
    pub fn preset_rule_id(mut self, id: u32) -> Self {
        self.preset_rule_id = id;
        self
    }

    /// Adds a preset application exclusion (e.g., WordPress, Nextcloud).
    pub fn add_preset(mut self, preset: Preset) -> Self {
        if !self.presets.contains(&preset) {
            self.presets.push(preset);
        }
        self
    }

    /// Adds a global rule removal by ID (`SecRuleRemoveById`).
    pub fn remove_rule_by_id(mut self, id: u32) -> Self {
        if !self.global_removals.contains(&id) {
            self.global_removals.push(id);
        }
        self
    }

    /// Adds a custom rule to remove a rule by ID for a specific URI (`ctl:ruleRemoveById`).
    pub fn remove_rule_by_id_for_uri(
        mut self,
        uri: &str,
        builder_rule_id: u32,
        remove_id: u32,
    ) -> Self {
        self.custom_uri_rules.push(CustomUriRule {
            uri_match: uri.to_string(),
            rule_id: builder_rule_id,
            action: CustomAction::RemoveById(remove_id),
        });
        self
    }

    /// Adds a custom rule to remove a rule target by ID for a specific URI (`ctl:ruleRemoveTargetById`).
    pub fn remove_target_by_id_for_uri(
        mut self,
        uri: &str,
        builder_rule_id: u32,
        remove_id: u32,
        target: &str,
    ) -> Self {
        self.custom_uri_rules.push(CustomUriRule {
            uri_match: uri.to_string(),
            rule_id: builder_rule_id,
            action: CustomAction::RemoveTargetById(remove_id, target.to_string()),
        });
        self
    }

    /// Builds the ModSecurity exclusion rule block as a String.
    pub fn build(&self) -> String {
        let mut output = String::new();

        if !self.presets.is_empty() {
            let mut actions = Vec::new();
            for preset in &self.presets {
                actions.push(preset.tx_variable().to_string());
            }
            writeln!(
                &mut output,
                "SecAction \"id:{},phase:1,nolog,pass,t:none,{}\"",
                self.preset_rule_id,
                actions.join(",")
            )
            .unwrap();
        }

        if !self.global_removals.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            let ids: Vec<String> = self.global_removals.iter().map(|id| id.to_string()).collect();
            writeln!(&mut output, "SecRuleRemoveById {}", ids.join(" ")).unwrap();
        }

        for rule in &self.custom_uri_rules {
            if !output.is_empty() {
                output.push('\n');
            }
            match &rule.action {
                CustomAction::RemoveById(remove_id) => {
                    writeln!(
                        &mut output,
                        "SecRule REQUEST_URI \"@beginsWith {}\" \"id:{},phase:1,pass,nolog,ctl:ruleRemoveById={}\"",
                        rule.uri_match.replace('"', "\\\""),
                        rule.rule_id,
                        remove_id
                    )
                    .unwrap();
                }
                CustomAction::RemoveTargetById(remove_id, target) => {
                    writeln!(
                        &mut output,
                        "SecRule REQUEST_URI \"@beginsWith {}\" \"id:{},phase:1,pass,nolog,ctl:ruleRemoveTargetById={};{}\"",
                        rule.uri_match.replace('"', "\\\""),
                        rule.rule_id,
                        remove_id,
                        target.replace('"', "\\\"")
                    )
                    .unwrap();
                }
            }
        }

        output
    }
}
