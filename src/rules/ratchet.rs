use crate::config::{RuleConfig, Severity};
use crate::rules::{Rule, RuleBuildError, ScanContext, Violation};
use regex::Regex;

/// A ratchet rule that counts literal pattern occurrences across all files.
///
/// Each match is reported as a violation. The scan layer post-processes:
/// if total matches <= `max_count`, all violations are suppressed (the team
/// is under budget). If over `max_count`, all violations are kept.
#[derive(Debug)]
pub struct RatchetRule {
    id: String,
    severity: Severity,
    message: String,
    suggest: Option<String>,
    glob: Option<String>,
    pattern: String,
    max_count: usize,
    compiled_regex: Option<Regex>,
}

impl RatchetRule {
    pub fn new(config: &RuleConfig) -> Result<Self, RuleBuildError> {
        let pattern = config
            .pattern
            .as_ref()
            .filter(|p| !p.is_empty())
            .ok_or_else(|| RuleBuildError::MissingField(config.id.clone(), "pattern"))?
            .clone();

        let max_count = config
            .max_count
            .ok_or_else(|| RuleBuildError::MissingField(config.id.clone(), "max_count"))?;

        let compiled_regex = if config.regex {
            let re = Regex::new(&pattern)
                .map_err(|e| RuleBuildError::InvalidRegex(config.id.clone(), e))?;
            Some(re)
        } else {
            None
        };

        Ok(Self {
            id: config.id.clone(),
            severity: config.severity,
            message: config.message.clone(),
            suggest: config.suggest.clone(),
            glob: config.glob.clone(),
            pattern,
            max_count,
            compiled_regex,
        })
    }

    pub fn max_count(&self) -> usize {
        self.max_count
    }

    pub fn pattern(&self) -> &str {
        &self.pattern
    }
}

impl Rule for RatchetRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn severity(&self) -> Severity {
        self.severity
    }

    fn file_glob(&self) -> Option<&str> {
        self.glob.as_deref()
    }

    fn check_file(&self, ctx: &ScanContext) -> Vec<Violation> {
        let mut violations = Vec::new();

        for (line_idx, line) in ctx.content.lines().enumerate() {
            if let Some(ref re) = self.compiled_regex {
                // Regex mode
                for m in re.find_iter(line) {
                    violations.push(Violation {
                        rule_id: self.id.clone(),
                        severity: self.severity,
                        file: ctx.file_path.to_path_buf(),
                        line: Some(line_idx + 1),
                        column: Some(m.start() + 1),
                        message: self.message.clone(),
                        suggest: self.suggest.clone(),
                        source_line: Some(line.to_string()),
                        fix: None,
                    });
                }
            } else {
                // Literal mode
                let pattern = self.pattern.as_str();
                let pattern_len = pattern.len();
                let mut search_start = 0;
                while let Some(pos) = line[search_start..].find(pattern) {
                    let col = search_start + pos;
                    violations.push(Violation {
                        rule_id: self.id.clone(),
                        severity: self.severity,
                        file: ctx.file_path.to_path_buf(),
                        line: Some(line_idx + 1),
                        column: Some(col + 1),
                        message: self.message.clone(),
                        suggest: self.suggest.clone(),
                        source_line: Some(line.to_string()),
                        fix: None,
                    });
                    search_start = col + pattern_len;
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn make_config(pattern: Option<&str>, max_count: Option<usize>) -> RuleConfig {
        RuleConfig {
            id: "test-ratchet".into(),
            severity: Severity::Error,
            message: "legacy pattern found".into(),
            suggest: Some("use newApi() instead".into()),
            pattern: pattern.map(|s| s.to_string()),
            max_count,
            ..Default::default()
        }
    }

    #[test]
    fn basic_match() {
        let config = make_config(Some("legacyFetch("), Some(10));
        let rule = RatchetRule::new(&config).unwrap();
        let content = "let x = legacyFetch(url);\nlet y = newFetch(url);";
        let ctx = ScanContext {
            file_path: Path::new("test.ts"),
            content,
        };
        let violations = rule.check_file(&ctx);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, Some(1));
        assert_eq!(violations[0].column, Some(9));
    }

    #[test]
    fn multiple_matches_per_line() {
        let config = make_config(Some("TODO"), Some(5));
        let rule = RatchetRule::new(&config).unwrap();
        let content = "// TODO fix this TODO and that TODO";
        let ctx = ScanContext {
            file_path: Path::new("test.ts"),
            content,
        };
        let violations = rule.check_file(&ctx);
        assert_eq!(violations.len(), 3);
        assert_eq!(violations[0].column, Some(4));
        assert_eq!(violations[1].column, Some(18));
        assert_eq!(violations[2].column, Some(32));
    }

    #[test]
    fn no_matches() {
        let config = make_config(Some("legacyFetch("), Some(0));
        let rule = RatchetRule::new(&config).unwrap();
        let content = "let x = apiFetch(url);";
        let ctx = ScanContext {
            file_path: Path::new("test.ts"),
            content,
        };
        let violations = rule.check_file(&ctx);
        assert!(violations.is_empty());
    }

    #[test]
    fn column_accuracy() {
        let config = make_config(Some("bad("), Some(10));
        let rule = RatchetRule::new(&config).unwrap();
        let content = "    bad(x)";
        let ctx = ScanContext {
            file_path: Path::new("test.ts"),
            content,
        };
        let violations = rule.check_file(&ctx);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].column, Some(5)); // 1-indexed
    }

    #[test]
    fn missing_pattern_error() {
        let config = make_config(None, Some(10));
        let err = RatchetRule::new(&config).unwrap_err();
        assert!(
            matches!(err, RuleBuildError::MissingField(_, "pattern")),
            "expected MissingField for pattern, got {:?}",
            err
        );
    }

    #[test]
    fn empty_pattern_error() {
        let config = make_config(Some(""), Some(10));
        let err = RatchetRule::new(&config).unwrap_err();
        assert!(matches!(err, RuleBuildError::MissingField(_, "pattern")));
    }

    #[test]
    fn missing_max_count_error() {
        let config = make_config(Some("TODO"), None);
        let err = RatchetRule::new(&config).unwrap_err();
        assert!(matches!(err, RuleBuildError::MissingField(_, "max_count")));
    }

    #[test]
    fn max_count_zero_works() {
        let config = make_config(Some("bad"), Some(0));
        let rule = RatchetRule::new(&config).unwrap();
        assert_eq!(rule.max_count(), 0);
    }

    #[test]
    fn accessors() {
        let config = make_config(Some("legacyFetch("), Some(47));
        let rule = RatchetRule::new(&config).unwrap();
        assert_eq!(rule.pattern(), "legacyFetch(");
        assert_eq!(rule.max_count(), 47);
        assert_eq!(rule.id(), "test-ratchet");
    }
}
