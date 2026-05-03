mod connect;

use std::{collections::HashSet, path::PathBuf};

use anyhow::{Result, bail};
use clap::Parser;

use self::connect::{check_alerts, check_databases};
use crate::{
    alerts::config::AlertsConfig,
    config::{Config, load_rules},
    pipeline::config::{DatabaseConfig, StoreType},
    rules::{
        config::{RuleConfig, RuleType},
        script,
    },
    validation::Validate,
};

#[derive(Parser)]
pub struct Args {
    /// Path to the config TOML to validate.
    #[arg(long, short = 'c', value_name = "FILE")]
    pub config: PathBuf,

    /// Optional rules file (overrides `rules_file` in config).
    #[arg(long, short = 'r', value_name = "FILE")]
    pub rules: Option<PathBuf>,

    /// Test live connectivity to databases and alert endpoints.
    #[arg(long)]
    pub connect: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity {
    Ok,
    Warning,
    Error,
}

#[derive(Debug)]
pub struct ValidationIssue {
    pub phase: &'static str,
    pub severity: IssueSeverity,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    fn push(&mut self, phase: &'static str, severity: IssueSeverity, message: impl Into<String>) {
        self.issues.push(ValidationIssue {
            phase,
            severity,
            message: message.into(),
        });
    }

    pub(crate) fn ok(&mut self, phase: &'static str, message: impl Into<String>) {
        self.push(phase, IssueSeverity::Ok, message);
    }

    pub(crate) fn warn(&mut self, phase: &'static str, message: impl Into<String>) {
        self.push(phase, IssueSeverity::Warning, message);
    }

    pub(crate) fn error(&mut self, phase: &'static str, message: impl Into<String>) {
        self.push(phase, IssueSeverity::Error, message);
    }

    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Warning)
            .count()
    }
}

pub async fn run(args: Args) -> Result<()> {
    println!("Validating configuration...\n");

    let mut report = ValidationReport::default();

    let config = match Config::load(Some(&args.config)) {
        Ok(cfg) => {
            report.ok(
                "config",
                format!(
                    "parsed successfully ({} databases, store={})",
                    cfg.databases.len(),
                    cfg.pipeline.store,
                ),
            );
            Some(cfg)
        },
        Err(e) => {
            report.error("config", format!("{e:#}"));
            None
        },
    };

    if let Some(config) = config.as_ref() {
        validate_databases(&config.databases, &mut report);
        validate_store(&config.pipeline.store, &mut report);
        validate_rules(args.rules.as_deref(), config, &mut report);
        validate_alerts(&config.alerts, &mut report);
        if args.connect {
            check_alerts(&config.alerts, &mut report).await;
            check_databases(&config.databases, &mut report).await;
        }
    }

    print_report(&report);

    let errors = report.error_count();
    let warnings = report.warning_count();
    println!("\nValidation complete: {errors} errors, {warnings} warnings");

    if errors > 0 {
        bail!("validation failed with {errors} error(s)");
    }
    Ok(())
}

fn print_report(report: &ValidationReport) {
    for issue in &report.issues {
        let tag = match issue.severity {
            IssueSeverity::Ok => "[OK]   ",
            IssueSeverity::Warning => "[WARN] ",
            IssueSeverity::Error => "[ERROR]",
        };
        println!("{tag} {}: {}", issue.phase, issue.message);
    }
}

fn validate_databases(databases: &[DatabaseConfig], report: &mut ValidationReport) {
    if databases.is_empty() {
        report.error("databases", "no databases configured");
        return;
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut had_error = false;
    for db in databases {
        let id = db.database_id();
        if !seen.insert(id.clone()) {
            report.warn(
                "databases",
                format!("duplicate database '{id}' — host/dbname combination must be unique"),
            );
        }
        for msg in db.validate(&id) {
            report.error("databases", msg);
            had_error = true;
        }
    }

    if !had_error {
        let mut sorted: Vec<&str> = seen.iter().map(String::as_str).collect();
        sorted.sort_unstable();
        report.ok(
            "databases",
            format!("{} database(s) configured ({})", databases.len(), sorted.join(", ")),
        );
    }
}

fn validate_store(store: &StoreType, report: &mut ValidationReport) {
    match store {
        StoreType::Memory => report.ok("store", "memory (state lost on restart)"),
        StoreType::Postgres => report.ok("store", "postgres (state persisted in source DB under `etl` schema)"),
    }
}

fn validate_alerts(alerts: &AlertsConfig, report: &mut ValidationReport) {
    let mut seen: HashSet<String> = HashSet::new();
    for (name, _) in alerts.channels() {
        if !seen.insert(name.clone()) {
            report.warn(
                "alerts",
                format!("duplicate alert channel name '{name}' — routing may be ambiguous"),
            );
        }
    }

    let errors = alerts.validate();
    let has_errors = !errors.is_empty();
    for msg in errors {
        report.error("alerts", msg);
    }

    if !has_errors {
        let mut sorted: Vec<&str> = seen.iter().map(String::as_str).collect();
        sorted.sort_unstable();
        report.ok("alerts", format!("{} channels configured ({})", seen.len(), sorted.join(", ")));
    }
}

fn validate_rules(rules_override: Option<&std::path::Path>, config: &Config, report: &mut ValidationReport) {
    let rules_path = match rules_override.or(config.rules_file.as_deref()) {
        Some(p) => p,
        None => {
            report.warn("rules", "no rules file specified — skipping rule validation");
            return;
        },
    };

    let rules = match load_rules(rules_path) {
        Ok(r) => r,
        Err(e) => {
            report.error("rules", format!("{e:#}"));
            return;
        },
    };

    let mut compiled_ok = 0;
    let (mut regex, mut builtin, mut script) = (0u32, 0u32, 0u32);

    for rule in &rules {
        match rule.rule_type {
            RuleType::Regex => regex += 1,
            RuleType::Builtin => builtin += 1,
            RuleType::Script => script += 1,
        }
        match validate_rule(rule) {
            Ok(()) => compiled_ok += 1,
            Err(e) => report.error("rules", format!("rule '{}': {e:#}", rule.id)),
        }
    }

    report.ok(
        "rules",
        format!(
            "{compiled_ok}/{} rules compiled ({regex} regex, {builtin} builtin, {script} script)",
            rules.len(),
        ),
    );

    let configured = config.alerts.names();
    for rule in &rules {
        if let Some(channels) = &rule.channels {
            for ch in channels {
                if !configured.contains(ch.as_str()) {
                    report.warn(
                        "rules",
                        format!("rule '{}' routes to channel '{ch}' which is not configured", rule.id),
                    );
                }
            }
        }
    }
}

fn validate_rule(rule: &RuleConfig) -> Result<()> {
    use anyhow::Context;

    if let Some(scope) = &rule.scope {
        scope.validate(&rule.id)?;
    }

    if let Some(al) = &rule.allowlist {
        regex::RegexSet::new(&al.patterns).context("invalid allowlist pattern")?;
    }

    match rule.rule_type {
        RuleType::Regex => {
            let pattern = rule
                .pattern
                .as_deref()
                .context("regex rule requires a `pattern` field")?;
            regex::Regex::new(pattern).context("invalid regex pattern")?;
        },
        RuleType::Builtin => {
            rule.builtin
                .ok_or_else(|| anyhow::anyhow!("builtin rule requires a `builtin` field"))?;
        },
        RuleType::Script => {
            let script_engine = script::create_script_engine();

            let path = rule
                .script
                .as_ref()
                .context("script rule requires a `script` field")?;
            script::compile_script(&script_engine, path).context("script compilation failed")?;
        },
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::config::Severity;

    fn rule(id: &str) -> RuleConfig {
        RuleConfig {
            id: id.into(),
            description: "x".into(),
            category: "TEST".into(),
            severity: Severity::Medium,
            rule_type: RuleType::Regex,
            pattern: None,
            validate: None,
            builtin: None,
            script: None,
            allowlist: None,
            scope: None,
            channels: None,
        }
    }

    mod rule {
        use super::*;

        #[test]
        fn regex_ok() {
            let r = RuleConfig {
                pattern: Some(r"\b\d{3}\b".into()),
                ..rule("ok")
            };
            validate_rule(&r).unwrap();
        }

        #[test]
        fn regex_invalid_pattern() {
            let r = RuleConfig {
                pattern: Some("[invalid".into()),
                ..rule("bad")
            };
            let err = validate_rule(&r).unwrap_err();
            assert!(err.to_string().contains("invalid regex"));
        }

        #[test]
        fn regex_missing_pattern() {
            let r = rule("missing");
            let err = validate_rule(&r).unwrap_err();
            assert!(err.to_string().contains("requires a `pattern` field"));
        }

        #[test]
        fn builtin_missing_builtin_field() {
            let r = RuleConfig {
                rule_type: RuleType::Builtin,
                ..rule("b")
            };
            let err = validate_rule(&r).unwrap_err();
            assert!(err.to_string().contains("requires a `builtin` field"));
        }

        #[test]
        fn script_missing_script_field() {
            let r = RuleConfig {
                rule_type: RuleType::Script,
                ..rule("s")
            };
            let err = validate_rule(&r).unwrap_err();
            assert!(err.to_string().contains("requires a `script` field"));
        }

        #[test]
        fn invalid_allowlist_pattern() {
            let r = RuleConfig {
                pattern: Some(r"\b\d+\b".into()),
                allowlist: Some(crate::rules::config::Allowlist {
                    description: None,
                    values: vec![],
                    patterns: vec!["[bad".into()],
                }),
                ..rule("al")
            };
            let err = validate_rule(&r).unwrap_err();
            assert!(err.to_string().contains("invalid allowlist pattern"));
        }
    }

    mod rules {
        use tempfile::NamedTempFile;

        use super::*;

        fn write_rule_file(file: &NamedTempFile, rules: &[(&str, &str)]) {
            let mut content = String::new();
            for (id, body) in rules {
                content.push_str(&format!(
                    "[[rules]]\nid = \"{id}\"\ndescription = \"x\"\ncategory = \"TEST\"\nseverity = \"medium\"\n{body}\n"
                ));
            }
            std::fs::write(file.path(), content).unwrap();
        }

        #[test]
        fn emits_warning_for_unknown_channel() {
            let mut config = Config::default();
            let rules_file = NamedTempFile::new().unwrap();
            write_rule_file(
                &rules_file,
                &[(
                    "r1",
                    r#"type = "regex"
pattern = "x"
channels = ["nonexistent"]
"#,
                )],
            );
            config.rules_file = Some(rules_file.path().to_path_buf());

            let mut report = ValidationReport::default();
            validate_rules(None, &config, &mut report);

            assert!(
                report
                    .issues
                    .iter()
                    .any(|i| { i.severity == IssueSeverity::Warning && i.message.contains("nonexistent") })
            );
        }
    }
}
