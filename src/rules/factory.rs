use crate::config::RuleConfig;
use crate::rules::tailwind_dark_mode::TailwindDarkModeRule;
use crate::rules::tailwind_theme_tokens::TailwindThemeTokensRule;
use crate::rules::{Rule, RuleBuildError};
use std::fmt;

#[derive(Debug)]
pub enum FactoryError {
    UnknownRuleType(String),
    BuildError(RuleBuildError),
}

impl fmt::Display for FactoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FactoryError::UnknownRuleType(t) => write!(f, "unknown rule type: '{}'", t),
            FactoryError::BuildError(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for FactoryError {}

impl From<RuleBuildError> for FactoryError {
    fn from(e: RuleBuildError) -> Self {
        FactoryError::BuildError(e)
    }
}

/// Build a rule instance from a type string and config.
pub fn build_rule(rule_type: &str, config: &RuleConfig) -> Result<Box<dyn Rule>, FactoryError> {
    match rule_type {
        "tailwind-dark-mode" => Ok(Box::new(TailwindDarkModeRule::new(config)?)),
        "tailwind-theme-tokens" => Ok(Box::new(TailwindThemeTokensRule::new(config)?)),
        _ => Err(FactoryError::UnknownRuleType(rule_type.to_string())),
    }
}
