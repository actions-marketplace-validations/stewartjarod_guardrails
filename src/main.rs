use clap::Parser;
use guardrails::cli::format;
use guardrails::cli::{Cli, Commands, OutputFormat};
use guardrails::config::Severity;
use guardrails::scan;
use std::process;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan {
            paths,
            config,
            format: output_format,
        } => {
            let result = match scan::run_scan(&config, &paths) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("\x1b[31merror\x1b[0m: {}", e);
                    process::exit(2);
                }
            };

            match output_format {
                OutputFormat::Pretty => format::print_pretty(&result),
                OutputFormat::Json => format::print_json(&result),
            }

            let has_errors = result
                .violations
                .iter()
                .any(|v| v.severity == Severity::Error);

            process::exit(if has_errors { 1 } else { 0 });
        }
    }
}
