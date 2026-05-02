use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::{
    config::Config,
    rules::{engine::RuleEngine, masking},
};

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    pub command: RulesCommand,

    #[arg(long, short = 'c', value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Path to custom rules TOML file (overrides config rules_file)
    #[arg(long, short = 'r', value_name = "FILE")]
    pub rules: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum RulesCommand {
    /// List all active detection rules
    List,
    /// Test a value against all detection rules
    Test {
        /// The value to test
        #[arg(long)]
        input: String,
    },
}

pub fn run(args: Args) -> Result<()> {
    let config: Config = crate::config::load(args.config.as_deref()).context("failed to load configuration")?;

    let rules_path = args
        .rules
        .as_deref()
        .or(config.rules_file.as_deref())
        .context("no rules file specified — use --rules <FILE> or set rules_file in config")?;
    let rules = crate::config::load_rules(rules_path).context("failed to load rules")?;

    match args.command {
        RulesCommand::List => list_rules(&rules),
        RulesCommand::Test { input } => test_value(&rules, &input),
    }
}

fn list_rules(rules: &[crate::rules::config::RuleConfig]) -> Result<()> {
    println!("{:<25} {:<8} {:<10} {:<12} DESCRIPTION", "ID", "TYPE", "SEVERITY", "CATEGORY");
    println!("{}", "-".repeat(85));

    for rule in rules {
        println!(
            "{:<25} {:<8} {:<10} {:<12} {}",
            rule.id, rule.rule_type, rule.severity, rule.category, rule.description
        );
    }

    println!("\n{} rules loaded", rules.len());
    Ok(())
}

fn test_value(rules: &[crate::rules::config::RuleConfig], input: &str) -> Result<()> {
    let engine = RuleEngine::new(rules, false).context("failed to compile detection rules")?;
    let matches = engine.scan_value(input);

    if matches.is_empty() {
        println!("No rules matched for input: {input}");
        return Ok(());
    }

    println!("Matches for input: {input}\n");
    for m in &matches {
        let masked = masking::mask(&m.matched_text);
        println!("  Rule:     {}", m.rule.id);
        println!("  Severity: {}", m.rule.severity);
        println!("  Category: {}", m.rule.category);
        println!("  Matched:  {}", masked);
        println!();
    }

    println!("{} rule(s) matched", matches.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::config::{RuleConfig, Severity};

    fn test_rules() -> Vec<RuleConfig> {
        vec![RuleConfig {
            id: "test-rule".into(),
            description: "Matches SECRET marker".into(),
            category: "TEST".into(),
            severity: Severity::Medium,
            rule_type: Default::default(),
            pattern: Some(r"SECRET-\d+".into()),
            validate: None,
            builtin: None,
            script: None,
            allowlist: None,
            scope: None,
            channels: None,
        }]
    }

    #[test]
    fn list_rules_succeeds() {
        list_rules(&test_rules()).unwrap();
    }

    #[test]
    fn test_value_match() {
        test_value(&test_rules(), "contains SECRET-42 here").unwrap();
    }

    #[test]
    fn test_value_no_match() {
        test_value(&test_rules(), "nothing here").unwrap();
    }
}
