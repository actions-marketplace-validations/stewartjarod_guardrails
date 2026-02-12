use crate::config::{RuleConfig, Severity};
use serde::Deserialize;

/// Top-level TOML config file structure.
#[derive(Debug, Deserialize)]
pub struct TomlConfig {
    pub guardrails: GuardrailsSection,
    #[serde(default)]
    pub rule: Vec<TomlRule>,
}

/// The `[guardrails]` section.
#[derive(Debug, Deserialize)]
pub struct GuardrailsSection {
    #[allow(dead_code)]
    pub name: Option<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// A single `[[rule]]` entry.
#[derive(Debug, Deserialize)]
pub struct TomlRule {
    pub id: String,
    #[serde(rename = "type")]
    pub rule_type: String,
    #[serde(default = "default_severity")]
    pub severity: String,
    pub glob: Option<String>,
    #[serde(default)]
    pub message: String,
    pub suggest: Option<String>,
    #[serde(default)]
    pub allowed_classes: Vec<String>,
    #[serde(default)]
    pub token_map: Vec<String>,
}

fn default_severity() -> String {
    "warning".into()
}

impl TomlRule {
    /// Convert to the core `RuleConfig` type.
    pub fn to_rule_config(&self) -> RuleConfig {
        let severity = match self.severity.to_lowercase().as_str() {
            "error" => Severity::Error,
            _ => Severity::Warning,
        };

        RuleConfig {
            id: self.id.clone(),
            severity,
            message: self.message.clone(),
            suggest: self.suggest.clone(),
            glob: self.glob.clone(),
            allowed_classes: self.allowed_classes.clone(),
            token_map: self.token_map.clone(),
        }
    }
}
