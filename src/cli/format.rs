use crate::config::Severity;
use crate::rules::Violation;
use crate::scan::ScanResult;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;

/// Print violations grouped by file with ANSI colors.
pub fn print_pretty(result: &ScanResult) {
    if result.violations.is_empty() {
        println!(
            "\x1b[32m✓\x1b[0m No violations found ({} files scanned, {} rules loaded)",
            result.files_scanned, result.rules_loaded
        );
        print_ratchet_summary(&result.ratchet_counts);
        return;
    }

    // Group violations by file
    let mut by_file: BTreeMap<String, Vec<&Violation>> = BTreeMap::new();
    for v in &result.violations {
        by_file
            .entry(v.file.display().to_string())
            .or_default()
            .push(v);
    }

    for (file, violations) in &by_file {
        println!("\n\x1b[4m{}\x1b[0m", file);
        for v in violations {
            let severity_str = match v.severity {
                Severity::Error => "\x1b[31merror\x1b[0m",
                Severity::Warning => "\x1b[33mwarn \x1b[0m",
            };

            let location = match (v.line, v.column) {
                (Some(l), Some(c)) => format!("{}:{}", l, c),
                (Some(l), None) => format!("{}:1", l),
                _ => "1:1".to_string(),
            };

            println!(
                "  \x1b[90m{:<8}\x1b[0m {} \x1b[90m{:<25}\x1b[0m {}",
                location, severity_str, v.rule_id, v.message
            );

            if let Some(ref source) = v.source_line {
                println!("           \x1b[90m│\x1b[0m {}", source.trim());
            }

            if let Some(ref suggest) = v.suggest {
                println!("           \x1b[90m└─\x1b[0m \x1b[36m{}\x1b[0m", suggest);
            }
        }
    }

    let errors = result
        .violations
        .iter()
        .filter(|v| v.severity == Severity::Error)
        .count();
    let warnings = result
        .violations
        .iter()
        .filter(|v| v.severity == Severity::Warning)
        .count();

    println!();
    print!("\x1b[1m");
    if errors > 0 {
        print!("\x1b[31m{} error{}\x1b[0m\x1b[1m", errors, if errors == 1 { "" } else { "s" });
    }
    if errors > 0 && warnings > 0 {
        print!(", ");
    }
    if warnings > 0 {
        print!("\x1b[33m{} warning{}\x1b[0m\x1b[1m", warnings, if warnings == 1 { "" } else { "s" });
    }
    println!(
        " ({} files scanned, {} rules loaded)\x1b[0m",
        result.files_scanned, result.rules_loaded
    );

    print_ratchet_summary(&result.ratchet_counts);
}

fn print_ratchet_summary(ratchet_counts: &HashMap<String, (usize, usize)>) {
    if ratchet_counts.is_empty() {
        return;
    }

    println!("\n\x1b[1mRatchet rules:\x1b[0m");
    let mut sorted: Vec<_> = ratchet_counts.iter().collect();
    sorted.sort_by_key(|(id, _)| (*id).clone());

    for (rule_id, &(found, max)) in &sorted {
        let status = if found <= max {
            format!("\x1b[32m✓ pass\x1b[0m ({}/{})", found, max)
        } else {
            format!("\x1b[31m✗ OVER\x1b[0m ({}/{})", found, max)
        };
        println!("  {:<30} {}", rule_id, status);
    }
}

/// Print violations as structured JSON.
pub fn print_json(result: &ScanResult) {
    let violations: Vec<_> = result
        .violations
        .iter()
        .map(|v| {
            json!({
                "rule_id": v.rule_id,
                "severity": match v.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                },
                "file": v.file.display().to_string(),
                "line": v.line,
                "column": v.column,
                "message": v.message,
                "suggest": v.suggest,
                "source_line": v.source_line,
            })
        })
        .collect();

    let ratchet: serde_json::Map<String, serde_json::Value> = result
        .ratchet_counts
        .iter()
        .map(|(id, &(found, max))| {
            (
                id.clone(),
                json!({ "found": found, "max": max, "pass": found <= max }),
            )
        })
        .collect();

    let output = json!({
        "violations": violations,
        "summary": {
            "total": result.violations.len(),
            "errors": result.violations.iter().filter(|v| v.severity == Severity::Error).count(),
            "warnings": result.violations.iter().filter(|v| v.severity == Severity::Warning).count(),
            "files_scanned": result.files_scanned,
            "rules_loaded": result.rules_loaded,
        },
        "ratchet": ratchet,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
