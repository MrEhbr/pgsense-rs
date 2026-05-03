use std::{
    hint::black_box,
    io::{BufRead, BufReader},
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use rand::{Rng, distr::Alphanumeric, rng};
use serde::Serialize;

use crate::{
    config::Config,
    rules::{
        config::{RuleConfig, RuleType},
        engine::RuleEngine,
        masking,
    },
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
    /// Benchmark per-rule scanning performance
    Bench(BenchArgs),
}

#[derive(Parser)]
pub struct BenchArgs {
    /// Single value to benchmark against
    #[arg(long, group = "input_source")]
    pub input: Option<String>,

    /// File with one value per line
    #[arg(long, value_name = "PATH", group = "input_source")]
    pub file: Option<PathBuf>,

    /// Generate N random test values
    #[arg(long, value_name = "COUNT", group = "input_source", default_value_t = 100)]
    pub generate: usize,

    /// Iterations per rule
    #[arg(long, default_value_t = 1000)]
    pub iterations: usize,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
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
        RulesCommand::Bench(bench) => bench_rules(&rules, bench),
    }
}

fn list_rules(rules: &[RuleConfig]) -> Result<()> {
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

fn test_value(rules: &[RuleConfig], input: &str) -> Result<()> {
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

fn bench_rules(rules: &[RuleConfig], args: BenchArgs) -> Result<()> {
    if args.iterations == 0 {
        bail!("--iterations must be > 0");
    }
    if rules.is_empty() {
        bail!("no rules loaded — nothing to benchmark");
    }

    let values = bench_inputs(&args)?;
    if values.is_empty() {
        bail!("no values to benchmark — file or generated set was empty");
    }

    let mut per_rule = Vec::with_capacity(rules.len());
    for rule in rules {
        let engine = RuleEngine::new(std::slice::from_ref(rule), false).with_context(|| format!("failed to compile rule '{}'", rule.id))?;
        let matches: usize = values.iter().map(|v| engine.scan_value(v).len()).sum();
        per_rule.push(RuleBenchResult {
            rule_id: rule.id.clone(),
            rule_type: rule.rule_type,
            stats: measure(&engine, &values, args.iterations),
            matches,
        });
    }
    per_rule.sort_by_key(|r| std::cmp::Reverse(r.stats.mean));

    let combined = RuleEngine::new(rules, false).context("failed to compile combined engine")?;
    let engine = measure(&combined, &values, args.iterations);

    let report = BenchReport {
        rules: per_rule,
        engine,
        config: BenchConfig {
            values: values.len(),
            iterations: args.iterations,
            rules: rules.len(),
        },
    };

    match args.format {
        OutputFormat::Table => print_table(&report),
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&report).context("failed to serialize bench report")?;
            println!("{json}");
        },
    }

    Ok(())
}

fn bench_inputs(args: &BenchArgs) -> Result<Vec<String>> {
    if let Some(v) = &args.input {
        return Ok(vec![v.clone()]);
    }
    if let Some(path) = &args.file {
        let f = std::fs::File::open(path).with_context(|| format!("failed to open input file: {}", path.display()))?;
        let lines: Result<Vec<String>, _> = BufReader::new(f).lines().collect();
        return lines.with_context(|| format!("failed to read input file: {}", path.display()));
    }
    if args.generate == 0 {
        bail!("--generate must be > 0");
    }
    const SAMPLE_CCS: &[&str] = &["4111111111111111", "5500000000000004", "340000000000009", "6011000000000004"];
    const SAMPLE_SSNS: &[&str] = &["123-45-6789", "001-01-0001", "555-12-3456"];
    const SAMPLE_PHONES: &[&str] = &["+44 20 7946 0958", "+1 415 555 2671", "+33 1 42 86 82 00"];
    const SAMPLE_AWS_KEYS: &[&str] = &["AKIAIOSFODNN7EXAMPLE", "AKIAJBCDEFGHIJKLMNOP"];
    const SAMPLE_GH_TOKENS: &[&str] = &["ghp_abcdefghijklmnopqrstuvwxyz0123456789", "gho_1234567890abcdefghijklmnopqrstuvwxyz"];

    let mut r = rng();
    let mut out = Vec::with_capacity(args.generate);
    for _ in 0..args.generate {
        let v = match r.random_range(0..100) {
            0..70 => {
                let len = r.random_range(20..80);
                (0..len)
                    .map(|_| {
                        if r.random_range(0..6) == 0 {
                            ' '
                        } else {
                            let c: u8 = r.sample(Alphanumeric);
                            c as char
                        }
                    })
                    .collect()
            },
            70..80 => SAMPLE_CCS[r.random_range(0..SAMPLE_CCS.len())].to_string(),
            80..85 => SAMPLE_SSNS[r.random_range(0..SAMPLE_SSNS.len())].to_string(),
            85..90 => SAMPLE_PHONES[r.random_range(0..SAMPLE_PHONES.len())].to_string(),
            90..95 => SAMPLE_AWS_KEYS[r.random_range(0..SAMPLE_AWS_KEYS.len())].to_string(),
            _ => SAMPLE_GH_TOKENS[r.random_range(0..SAMPLE_GH_TOKENS.len())].to_string(),
        };
        out.push(v);
    }
    Ok(out)
}

fn measure(engine: &RuleEngine, values: &[String], iterations: usize) -> DurationStats {
    let mut s = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        for v in values {
            black_box(engine.scan_value(black_box(v)));
        }
        s.push(start.elapsed());
    }
    s.sort();
    DurationStats {
        mean: s.iter().sum::<Duration>() / s.len() as u32,
        p50: percentile(&s, 50),
        p95: percentile(&s, 95),
        p99: percentile(&s, 99),
    }
}

fn percentile(sorted: &[Duration], p: usize) -> Duration {
    let idx = (sorted.len() * p / 100).min(sorted.len() - 1);
    sorted[idx]
}

fn fmt_duration(d: Duration) -> String {
    let us = d.as_secs_f64() * 1_000_000.0;
    if us >= 1_000_000.0 {
        format!("{:.2}s", us / 1_000_000.0)
    } else if us >= 1000.0 {
        format!("{:.2}ms", us / 1000.0)
    } else if us >= 1.0 {
        format!("{us:.1}us")
    } else {
        format!("{:.1}ns", us * 1000.0)
    }
}

fn ser_duration<S: serde::Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&fmt_duration(*d))
}

#[derive(Serialize)]
struct DurationStats {
    #[serde(serialize_with = "ser_duration")]
    mean: Duration,
    #[serde(serialize_with = "ser_duration")]
    p50: Duration,
    #[serde(serialize_with = "ser_duration")]
    p95: Duration,
    #[serde(serialize_with = "ser_duration")]
    p99: Duration,
}

#[derive(Serialize)]
struct RuleBenchResult {
    rule_id: String,
    rule_type: RuleType,
    #[serde(flatten)]
    stats: DurationStats,
    matches: usize,
}

#[derive(Serialize)]
struct BenchConfig {
    values: usize,
    iterations: usize,
    rules: usize,
}

#[derive(Serialize)]
struct BenchReport {
    rules: Vec<RuleBenchResult>,
    engine: DurationStats,
    config: BenchConfig,
}

fn print_table(report: &BenchReport) {
    println!("Rule Performance (sorted by mean, slowest first)\n");
    println!(
        "{:<25} {:<8} {:<10} {:<10} {:<10} {:<10} MATCHES",
        "RULE ID", "TYPE", "MEAN", "P50", "P95", "P99"
    );
    println!("{}", "-".repeat(89));
    for r in &report.rules {
        println!(
            "{:<25} {:<8} {:<10} {:<10} {:<10} {:<10} {}",
            r.rule_id,
            r.rule_type,
            fmt_duration(r.stats.mean),
            fmt_duration(r.stats.p50),
            fmt_duration(r.stats.p95),
            fmt_duration(r.stats.p99),
            r.matches,
        );
    }
    println!("\nEngine scan_value() (all rules combined):");
    println!(
        "  Mean: {}  P50: {}  P95: {}  P99: {}",
        fmt_duration(report.engine.mean),
        fmt_duration(report.engine.p50),
        fmt_duration(report.engine.p95),
        fmt_duration(report.engine.p99),
    );
    println!(
        "\nInput: {} values, {} iterations, {} rules",
        report.config.values, report.config.iterations, report.config.rules,
    );
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

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

    #[test]
    fn percentile_indexes_known_distribution() {
        let mut samples: Vec<Duration> = (1..=100).map(Duration::from_micros).collect();
        samples.sort();
        assert_eq!(percentile(&samples, 50), Duration::from_micros(51));
        assert_eq!(percentile(&samples, 95), Duration::from_micros(96));
        assert_eq!(percentile(&samples, 99), Duration::from_micros(100));
    }

    #[test]
    fn percentile_clamps_to_last_index() {
        let samples = vec![Duration::from_micros(7)];
        assert_eq!(percentile(&samples, 50), Duration::from_micros(7));
        assert_eq!(percentile(&samples, 99), Duration::from_micros(7));
    }

    #[test]
    fn fmt_duration_switches_units() {
        assert_eq!(fmt_duration(Duration::from_nanos(50)), "50.0ns");
        assert_eq!(fmt_duration(Duration::from_nanos(500)), "500.0ns");
        assert_eq!(fmt_duration(Duration::from_nanos(12_300)), "12.3us");
        assert_eq!(fmt_duration(Duration::from_nanos(999_900)), "999.9us");
        assert_eq!(fmt_duration(Duration::from_micros(1500)), "1.50ms");
        assert_eq!(fmt_duration(Duration::from_nanos(12_345_000)), "12.35ms");
        assert_eq!(fmt_duration(Duration::from_micros(1_500_000)), "1.50s");
        assert_eq!(fmt_duration(Duration::from_nanos(123_456_789_000)), "123.46s");
    }

    #[test]
    fn bench_rules_runs_minimum() {
        let rules = test_rules();
        let args = BenchArgs {
            input: Some("foo SECRET-1 bar".into()),
            file: None,
            generate: 100,
            iterations: 5,
            format: OutputFormat::Json,
        };
        bench_rules(&rules, args).unwrap();
    }
}
