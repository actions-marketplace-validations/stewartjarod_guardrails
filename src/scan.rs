use crate::cli::toml_config::TomlConfig;
use crate::rules::factory::{self, FactoryError};
use crate::rules::{Rule, ScanContext, Violation};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug)]
pub enum ScanError {
    ConfigRead(std::io::Error),
    ConfigParse(toml::de::Error),
    GlobParse(globset::Error),
    RuleFactory(FactoryError),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::ConfigRead(e) => write!(f, "failed to read config: {}", e),
            ScanError::ConfigParse(e) => write!(f, "failed to parse config: {}", e),
            ScanError::GlobParse(e) => write!(f, "invalid glob pattern: {}", e),
            ScanError::RuleFactory(e) => write!(f, "failed to build rule: {}", e),
        }
    }
}

impl std::error::Error for ScanError {}

pub struct ScanResult {
    pub violations: Vec<Violation>,
    pub files_scanned: usize,
    pub rules_loaded: usize,
}

/// Run a full scan: parse config, build rules, walk files, collect violations.
pub fn run_scan(config_path: &Path, target_paths: &[PathBuf]) -> Result<ScanResult, ScanError> {
    // 1. Read and parse TOML config
    let config_text = fs::read_to_string(config_path).map_err(ScanError::ConfigRead)?;
    let toml_config: TomlConfig = toml::from_str(&config_text).map_err(ScanError::ConfigParse)?;

    // 2. Build exclude glob set
    // Include patterns are advisory for project-wide scanning; CLI-provided targets
    // override them (the user explicitly chose what to scan). Exclude patterns still
    // apply to skip directories like node_modules.
    let exclude_set = build_glob_set(&toml_config.guardrails.exclude)?;

    // 3. Build rules via factory
    let mut rules: Vec<(Box<dyn Rule>, Option<GlobSet>)> = Vec::new();
    for toml_rule in &toml_config.rule {
        let rule_config = toml_rule.to_rule_config();
        let rule = factory::build_rule(&toml_rule.rule_type, &rule_config)
            .map_err(ScanError::RuleFactory)?;

        // Build per-rule glob if specified
        let rule_glob = if let Some(ref pattern) = rule.file_glob() {
            let gs = GlobSetBuilder::new()
                .add(Glob::new(pattern).map_err(ScanError::GlobParse)?)
                .build()
                .map_err(ScanError::GlobParse)?;
            Some(gs)
        } else {
            None
        };

        rules.push((rule, rule_glob));
    }

    let rules_loaded = rules.len();

    // 4. Walk target paths and collect files
    let mut files: Vec<PathBuf> = Vec::new();
    for target in target_paths {
        if target.is_file() {
            files.push(target.clone());
        } else {
            for entry in WalkDir::new(target).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let path = entry.into_path();

                    // Apply exclude patterns against the path relative to target
                    let rel = path.strip_prefix(target).unwrap_or(&path);
                    if exclude_set.is_match(rel.to_string_lossy().as_ref()) {
                        continue;
                    }

                    files.push(path);
                }
            }
        }
    }

    // 5. Run rules on each file
    let mut violations: Vec<Violation> = Vec::new();
    let mut files_scanned = 0;

    for file_path in &files {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue, // skip binary/unreadable files
        };

        files_scanned += 1;
        let ctx = ScanContext {
            file_path,
            content: &content,
        };

        for (rule, rule_glob) in &rules {
            // Apply per-rule glob filter
            if let Some(ref gs) = rule_glob {
                let file_str = file_path.to_string_lossy();
                let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();
                if !gs.is_match(&*file_str) && !gs.is_match(&*file_name) {
                    continue;
                }
            }

            let mut file_violations = rule.check_file(&ctx);
            violations.append(&mut file_violations);
        }
    }

    Ok(ScanResult {
        violations,
        files_scanned,
        rules_loaded,
    })
}

fn build_glob_set(patterns: &[String]) -> Result<GlobSet, ScanError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern).map_err(ScanError::GlobParse)?);
    }
    builder.build().map_err(ScanError::GlobParse)
}
