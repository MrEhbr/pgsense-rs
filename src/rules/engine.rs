use std::{borrow::Cow, collections::HashSet, time::Instant};

use anyhow::{Context, Result};
use prometheus::HistogramVec;
use regex::{Regex, RegexSet};
use tracing::{debug, warn};

use super::{
    config::{RuleConfig, RuleScope, RuleType, Severity, Validator},
    detectors::Detector,
    script, validators,
};
use crate::metrics;

pub struct RuleMetadata {
    pub id: String,
    pub description: String,
    pub category: String,
    pub severity: Severity,
    pub rule_type: RuleType,
    pub scope: Option<RuleScope>,
    pub channels: Option<Vec<String>>,
}

pub enum RuleKind {
    Regex { regex: Regex, validator: Option<Validator> },
    Builtin(Detector),
    Script { ast: rhai::AST },
}

pub struct CompiledAllowlist {
    description: Option<String>,
    values: HashSet<String>,
    patterns: RegexSet,
}

impl CompiledAllowlist {
    fn is_allowed(&self, rule_id: &str, text: &str) -> bool {
        let allowed = self.values.contains(text) || self.patterns.is_match(text);
        if allowed {
            debug!(
                rule = rule_id,
                matched_text = text,
                reason = self.description.as_deref().unwrap_or(""),
                "match suppressed by allowlist"
            );
        }
        allowed
    }
}

pub struct CompiledRule {
    pub meta: RuleMetadata,
    pub kind: RuleKind,
    allowlist: Option<CompiledAllowlist>,
}

impl CompiledRule {
    fn is_allowed(&self, text: &str) -> bool {
        self.allowlist
            .as_ref()
            .is_some_and(|al| al.is_allowed(&self.meta.id, text))
    }
}

pub struct RuleMatch<'a> {
    pub rule: &'a RuleMetadata,
    pub matched_text: Cow<'a, str>,
}

/// Multi-type rule engine with three-phase scanning.
pub struct RuleEngine {
    rules: Vec<CompiledRule>,
    regex_set: RegexSet,
    /// Maps regex_set match index → rules[] index
    regex_indices: Vec<usize>,
    builtin_indices: Vec<usize>,
    script_indices: Vec<usize>,
    script_engine: rhai::Engine,
    profiling_enabled: bool,
}

#[inline(always)]
fn mark<const P: bool>() -> Option<Instant> {
    if P { Some(Instant::now()) } else { None }
}

#[inline(always)]
fn record<const P: bool>(start: Option<Instant>, hist: &HistogramVec, label: &str) {
    if P {
        hist.with_label_values(&[label])
            .observe(start.unwrap().elapsed().as_secs_f64());
    }
}

impl RuleEngine {
    pub fn new(configs: &[RuleConfig], profiling_enabled: bool) -> Result<Self> {
        let script_engine = script::create_script_engine();
        let mut rules = Vec::with_capacity(configs.len());
        let mut regex_patterns: Vec<String> = Vec::new();
        let mut regex_indices: Vec<usize> = Vec::new();
        let mut builtin_indices: Vec<usize> = Vec::new();
        let mut script_indices: Vec<usize> = Vec::new();

        for (i, c) in configs.iter().enumerate() {
            if let Some(scope) = &c.scope {
                scope.validate(&c.id)?;
            }

            let meta = RuleMetadata {
                id: c.id.clone(),
                description: c.description.clone(),
                category: c.category.clone(),
                severity: c.severity,
                rule_type: c.rule_type,
                scope: c.scope.clone(),
                channels: c.channels.clone(),
            };

            let kind = match c.rule_type {
                RuleType::Regex => {
                    let pattern = c.pattern.as_deref().unwrap_or_else(|| {
                        panic!("regex rule '{}' requires a pattern", c.id);
                    });
                    let regex = Regex::new(pattern).with_context(|| format!("invalid regex pattern for rule '{}'", c.id))?;
                    regex_patterns.push(pattern.to_string());
                    regex_indices.push(i);
                    RuleKind::Regex { regex, validator: c.validate }
                },
                RuleType::Builtin => {
                    let builtin_kind = c.builtin.unwrap_or_else(|| {
                        panic!("builtin rule '{}' requires a `builtin` field", c.id);
                    });
                    builtin_indices.push(i);
                    RuleKind::Builtin(Detector::from_kind(builtin_kind))
                },
                RuleType::Script => {
                    let path = c.script.as_ref().unwrap_or_else(|| {
                        panic!("script rule '{}' requires a `script` field", c.id);
                    });
                    let ast = script::compile_script(&script_engine, path).with_context(|| format!("failed to compile script for rule '{}'", c.id))?;
                    script_indices.push(i);
                    RuleKind::Script { ast }
                },
            };

            let allowlist = match &c.allowlist {
                Some(al) => {
                    let values: HashSet<String> = al.values.iter().cloned().collect();
                    let patterns = RegexSet::new(&al.patterns).with_context(|| format!("invalid allowlist pattern for rule '{}'", c.id))?;
                    Some(CompiledAllowlist {
                        description: al.description.clone(),
                        values,
                        patterns,
                    })
                },
                None => None,
            };

            rules.push(CompiledRule { meta, kind, allowlist });
        }

        let regex_set = RegexSet::new(&regex_patterns).context("failed to compile regex set")?;

        Ok(Self {
            rules,
            regex_set,
            regex_indices,
            builtin_indices,
            script_indices,
            script_engine,
            profiling_enabled,
        })
    }

    pub fn scan_value<'a>(&'a self, value: &'a str) -> Vec<RuleMatch<'a>> {
        if self.profiling_enabled {
            self.scan_inner::<true>(value)
        } else {
            self.scan_inner::<false>(value)
        }
    }

    // Monomorphized: `P = false` const-folds every `if P` and `mark`/`record`
    // call to nothing, leaving an instrumentation-free scan body.
    fn scan_inner<'a, const P: bool>(&'a self, value: &'a str) -> Vec<RuleMatch<'a>> {
        let mut results = Vec::new();

        // Phase 1: RegexSet fast-path
        let phase_start = mark::<P>();
        for set_idx in self.regex_set.matches(value).into_iter() {
            let rule_idx = self.regex_indices[set_idx];
            let rule = &self.rules[rule_idx];
            let rule_start = mark::<P>();
            if let RuleKind::Regex { ref regex, ref validator } = rule.kind
                && let Some(mat) = regex.find(value)
            {
                let matched_text = mat.as_str();

                if let Some(v) = validator {
                    let valid = match v {
                        Validator::Luhn => validators::luhn(matched_text),
                        Validator::Ssn => validators::ssn(matched_text),
                    };
                    if !valid {
                        record::<P>(rule_start, &metrics::RULE_SCAN_DURATION, &rule.meta.id);
                        continue;
                    }
                }

                if rule.is_allowed(matched_text) {
                    record::<P>(rule_start, &metrics::RULE_SCAN_DURATION, &rule.meta.id);
                    continue;
                }

                results.push(RuleMatch {
                    rule: &rule.meta,
                    matched_text: Cow::Borrowed(matched_text),
                });
            }
            record::<P>(rule_start, &metrics::RULE_SCAN_DURATION, &rule.meta.id);
        }
        record::<P>(phase_start, &metrics::PHASE_SCAN_DURATION, "regex");

        // Phase 2: Builtin detectors
        let phase_start = mark::<P>();
        for &rule_idx in &self.builtin_indices {
            let rule = &self.rules[rule_idx];
            let RuleKind::Builtin(ref detector) = rule.kind else { continue };
            let rule_start = mark::<P>();
            for matched_text in detector.scan(value) {
                if rule.is_allowed(&matched_text) {
                    continue;
                }
                results.push(RuleMatch {
                    rule: &rule.meta,
                    matched_text: Cow::Owned(matched_text),
                });
            }
            record::<P>(rule_start, &metrics::RULE_SCAN_DURATION, &rule.meta.id);
        }
        record::<P>(phase_start, &metrics::PHASE_SCAN_DURATION, "builtin");

        // Phase 3: Script rules
        let phase_start = mark::<P>();
        for &rule_idx in &self.script_indices {
            let rule = &self.rules[rule_idx];
            let RuleKind::Script { ref ast } = rule.kind else { continue };
            let rule_start = mark::<P>();
            match script::run_detect(&self.script_engine, ast, value) {
                Ok(matches) => {
                    for matched_text in matches {
                        if rule.is_allowed(&matched_text) {
                            continue;
                        }
                        results.push(RuleMatch {
                            rule: &rule.meta,
                            matched_text: Cow::Owned(matched_text),
                        });
                    }
                },
                Err(e) => {
                    metrics::SCRIPT_ERRORS
                        .with_label_values(&[&rule.meta.id])
                        .inc();
                    warn!(rule_id = %rule.meta.id, error = %e, "script rule execution failed");
                },
            }
            record::<P>(rule_start, &metrics::RULE_SCAN_DURATION, &rule.meta.id);
        }
        record::<P>(phase_start, &metrics::PHASE_SCAN_DURATION, "script");

        results
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use rstest::rstest;

    use super::*;
    use crate::{
        pattern::PatternMatcher,
        rules::config::{Allowlist, BuiltinKind, RuleScope, Validator},
    };

    fn pm(patterns: &[&str]) -> PatternMatcher {
        PatternMatcher::compile(patterns.iter().map(|s| s.to_string()).collect()).unwrap()
    }

    fn cfg(id: &str) -> RuleConfig {
        RuleConfig {
            id: id.into(),
            description: format!("test: {id}"),
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

    fn tmp_script(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn empty_config_compiles_to_empty_engine() {
        let engine = RuleEngine::new(&[], false).unwrap();
        assert_eq!(engine.rule_count(), 0);
        assert!(engine.scan_value("anything").is_empty());
    }

    #[test]
    fn invalid_regex_rejected_at_compile() {
        assert!(
            RuleEngine::new(
                &[RuleConfig {
                    pattern: Some("[invalid".into()),
                    ..cfg("bad")
                }],
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn scope_table_in_both_include_and_exclude_rejected() {
        let err = RuleEngine::new(
            &[RuleConfig {
                pattern: Some(r"\bfoo\b".into()),
                scope: Some(RuleScope {
                    include_tables: pm(&["users"]),
                    exclude_tables: pm(&["users"]),
                    ..Default::default()
                }),
                ..cfg("bad-scope")
            }],
            false,
        )
        .err()
        .expect("should reject conflicting scope");
        let msg = err.to_string();
        assert!(msg.contains("users"), "error should name the table: {msg}");
        assert!(msg.contains("include_tables"), "error should name the field: {msg}");
    }

    #[test]
    fn scope_column_in_both_include_and_exclude_rejected() {
        let err = RuleEngine::new(
            &[RuleConfig {
                pattern: Some(r"\bfoo\b".into()),
                scope: Some(RuleScope {
                    include_columns: pm(&["email"]),
                    exclude_columns: pm(&["email"]),
                    ..Default::default()
                }),
                ..cfg("bad-scope")
            }],
            false,
        )
        .err()
        .expect("should reject conflicting scope");
        let msg = err.to_string();
        assert!(msg.contains("email"), "error should name the column: {msg}");
        assert!(msg.contains("include_columns"), "error should name the field: {msg}");
    }

    // Interleaving builtins must not corrupt the RegexSet→rules[] index mapping.
    #[test]
    fn regex_indices_skip_non_regex_rules() {
        let configs = vec![
            RuleConfig {
                rule_type: RuleType::Builtin,
                builtin: Some(BuiltinKind::CreditCard),
                ..cfg("b1")
            }, // rules[0]
            RuleConfig {
                pattern: Some(r"\baaa\b".into()),
                ..cfg("r1")
            }, // rules[1] → set[0]
            RuleConfig {
                rule_type: RuleType::Builtin,
                builtin: Some(BuiltinKind::Ssn),
                ..cfg("b2")
            }, // rules[2]
            RuleConfig {
                pattern: Some(r"\bbbb\b".into()),
                ..cfg("r2")
            }, // rules[3] → set[1]
        ];
        let engine = RuleEngine::new(&configs, false).unwrap();

        let m = engine.scan_value("aaa");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].rule.id, "r1");

        let m = engine.scan_value("bbb");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].rule.id, "r2");
    }

    #[test]
    fn all_three_phases_contribute_results() {
        let script = tmp_script(r#"fn detect(value) { if value.contains("KEYWORD") { ["KEYWORD"] } else { [] } }"#);
        let configs = vec![
            RuleConfig {
                pattern: Some(r"\bfoo\b".into()),
                ..cfg("r1")
            },
            RuleConfig {
                rule_type: RuleType::Builtin,
                builtin: Some(BuiltinKind::CreditCard),
                ..cfg("cc")
            },
            RuleConfig {
                rule_type: RuleType::Script,
                script: Some(script.path().into()),
                ..cfg("s1")
            },
        ];
        let engine = RuleEngine::new(&configs, false).unwrap();

        let m = engine.scan_value("foo 4111111111111111 KEYWORD");
        assert_eq!(m.len(), 3);
        let ids: Vec<&str> = m.iter().map(|m| m.rule.id.as_str()).collect();
        assert!(ids.contains(&"r1"), "regex phase missing");
        assert!(ids.contains(&"cc"), "builtin phase missing");
        assert!(ids.contains(&"s1"), "script phase missing");
    }

    #[rstest]
    #[case("4111111111111111", 1)]
    #[case("4111111111111112", 0)]
    fn validator_gates_regex_match(#[case] input: &str, #[case] expected: usize) {
        let configs = vec![RuleConfig {
            pattern: Some(r"\b[0-9]{13,19}\b".into()),
            validate: Some(Validator::Luhn),
            ..cfg("cc")
        }];
        let engine = RuleEngine::new(&configs, false).unwrap();
        assert_eq!(engine.scan_value(input).len(), expected);
    }

    #[test]
    fn matched_text_is_the_match_not_full_input() {
        let engine = RuleEngine::new(
            &[RuleConfig {
                pattern: Some(r"\b\d{3}\b".into()),
                ..cfg("r1")
            }],
            false,
        )
        .unwrap();
        let m = engine.scan_value("abc 123 def");
        assert_eq!(m[0].matched_text, "123");
    }

    #[rstest]
    #[case("exact_hit",    Some(Allowlist { description: None, values: vec!["noreply@example.com".into()], patterns: vec![] }), "noreply@example.com", 0)]
    #[case("exact_miss",   Some(Allowlist { description: None, values: vec!["noreply@example.com".into()], patterns: vec![] }), "user@example.com",    1)]
    #[case("pattern_hit",  Some(Allowlist { description: None, values: vec![], patterns: vec![r".*@internal\.corp".into()] }),  "alice@internal.corp",  0)]
    #[case("pattern_miss", Some(Allowlist { description: None, values: vec![], patterns: vec![r".*@internal\.corp".into()] }),  "alice@external.com",   1)]
    #[case("none", None, "user@example.com", 1)]
    fn allowlist_filters_regex(#[case] _label: &str, #[case] allowlist: Option<Allowlist>, #[case] input: &str, #[case] expected: usize) {
        let configs = vec![RuleConfig {
            pattern: Some(r"\b\w+@\w+\.\w+\b".into()),
            allowlist,
            ..cfg("email")
        }];
        let engine = RuleEngine::new(&configs, false).unwrap();
        assert_eq!(engine.scan_value(input).len(), expected);
    }

    #[rstest]
    #[case("allowed", "4111111111111111", 0)]
    #[case("not_allowed", "5500000000000004", 1)]
    fn allowlist_filters_builtin(#[case] _label: &str, #[case] input: &str, #[case] expected: usize) {
        let configs = vec![RuleConfig {
            rule_type: RuleType::Builtin,
            builtin: Some(BuiltinKind::CreditCard),
            allowlist: Some(Allowlist {
                description: None,
                values: vec!["4111111111111111".into()],
                patterns: vec![],
            }),
            ..cfg("cc")
        }];
        let engine = RuleEngine::new(&configs, false).unwrap();
        assert_eq!(engine.scan_value(input).len(), expected);
    }

    #[test]
    fn email_builtin_through_engine() {
        let configs = vec![RuleConfig {
            rule_type: RuleType::Builtin,
            builtin: Some(BuiltinKind::Email),
            ..cfg("email")
        }];
        let engine = RuleEngine::new(&configs, false).unwrap();

        let m = engine.scan_value("contact alice@example.com now");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].rule.id, "email");
        assert_eq!(m[0].matched_text, "alice@example.com");

        assert!(engine.scan_value("no email here").is_empty());
    }

    #[test]
    fn phone_builtin_through_engine() {
        let configs = vec![RuleConfig {
            rule_type: RuleType::Builtin,
            builtin: Some(BuiltinKind::Phone),
            ..cfg("phone")
        }];
        let engine = RuleEngine::new(&configs, false).unwrap();

        let m = engine.scan_value("call +44 20 7946 0958 now");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].rule.id, "phone");
        assert_eq!(m[0].matched_text, "+44 20 7946 0958");

        // bare digits should not match
        assert!(engine.scan_value("1234567890").is_empty());
    }
}
