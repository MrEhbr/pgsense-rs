use std::collections::HashSet;

use anyhow::{Context, Result};
use regex::{Regex, RegexSet};
use tracing::debug;

use super::{
    config::{RuleConfig, RuleType, Severity, Validator},
    detectors::Detector,
    script, validators,
};

pub struct RuleMetadata {
    pub id: String,
    pub description: String,
    pub category: String,
    pub severity: Severity,
    pub rule_type: RuleType,
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

pub struct RuleMatch<'a> {
    pub rule: &'a RuleMetadata,
    pub matched_text: String,
}

/// Multi-type rule engine with three-phase scanning.
pub struct RuleEngine {
    rules: Vec<CompiledRule>,
    regex_set: RegexSet,
    /// Maps regex_set match index → rules[] index
    regex_indices: Vec<usize>,
    script_engine: rhai::Engine,
}

impl RuleEngine {
    pub fn new(configs: &[RuleConfig]) -> Result<Self> {
        let script_engine = script::create_script_engine();
        let mut rules = Vec::with_capacity(configs.len());
        let mut regex_patterns: Vec<String> = Vec::new();
        let mut regex_indices: Vec<usize> = Vec::new();

        for (i, c) in configs.iter().enumerate() {
            let meta = RuleMetadata {
                id: c.id.clone(),
                description: c.description.clone(),
                category: c.category.clone(),
                severity: c.severity,
                rule_type: c.rule_type,
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
                    RuleKind::Builtin(Detector::from_kind(builtin_kind))
                },
                RuleType::Script => {
                    let path = c.script.as_ref().unwrap_or_else(|| {
                        panic!("script rule '{}' requires a `script` field", c.id);
                    });
                    let ast = script::compile_script(&script_engine, path).with_context(|| format!("failed to compile script for rule '{}'", c.id))?;
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
            script_engine,
        })
    }

    pub fn scan_value<'a>(&'a self, value: &str) -> Vec<RuleMatch<'a>> {
        let mut results = Vec::new();

        // Phase 1: RegexSet fast-path
        for set_idx in self.regex_set.matches(value).into_iter() {
            let rule_idx = self.regex_indices[set_idx];
            let rule = &self.rules[rule_idx];
            if let RuleKind::Regex { ref regex, ref validator } = rule.kind
                && let Some(mat) = regex.find(value)
            {
                let matched_text = mat.as_str().to_string();

                if let Some(v) = validator {
                    let valid = match v {
                        Validator::Luhn => validators::luhn(&matched_text),
                        Validator::Ssn => validators::ssn(&matched_text),
                    };
                    if !valid {
                        continue;
                    }
                }

                if rule
                    .allowlist
                    .as_ref()
                    .is_some_and(|al| al.is_allowed(&rule.meta.id, &matched_text))
                {
                    continue;
                }

                results.push(RuleMatch {
                    rule: &rule.meta,
                    matched_text,
                });
            }
        }

        // Phase 2: Builtin detectors
        for rule in &self.rules {
            if let RuleKind::Builtin(ref detector) = rule.kind {
                for matched_text in detector.scan(value) {
                    if rule
                        .allowlist
                        .as_ref()
                        .is_some_and(|al| al.is_allowed(&rule.meta.id, &matched_text))
                    {
                        continue;
                    }
                    results.push(RuleMatch {
                        rule: &rule.meta,
                        matched_text,
                    });
                }
            }
        }

        // Phase 3: Script rules
        for rule in &self.rules {
            if let RuleKind::Script { ref ast } = rule.kind
                && let Ok(matches) = script::run_detect(&self.script_engine, ast, value)
            {
                for matched_text in matches {
                    if rule
                        .allowlist
                        .as_ref()
                        .is_some_and(|al| al.is_allowed(&rule.meta.id, &matched_text))
                    {
                        continue;
                    }
                    results.push(RuleMatch {
                        rule: &rule.meta,
                        matched_text,
                    });
                }
            }
        }

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
    use crate::rules::config::{Allowlist, BuiltinKind, Validator};

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
        let engine = RuleEngine::new(&[]).unwrap();
        assert_eq!(engine.rule_count(), 0);
        assert!(engine.scan_value("anything").is_empty());
    }

    #[test]
    fn invalid_regex_rejected_at_compile() {
        assert!(
            RuleEngine::new(&[RuleConfig {
                pattern: Some("[invalid".into()),
                ..cfg("bad")
            }])
            .is_err()
        );
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
        let engine = RuleEngine::new(&configs).unwrap();

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
        let engine = RuleEngine::new(&configs).unwrap();

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
        let engine = RuleEngine::new(&configs).unwrap();
        assert_eq!(engine.scan_value(input).len(), expected);
    }

    #[test]
    fn matched_text_is_the_match_not_full_input() {
        let engine = RuleEngine::new(&[RuleConfig {
            pattern: Some(r"\b\d{3}\b".into()),
            ..cfg("r1")
        }])
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
        let engine = RuleEngine::new(&configs).unwrap();
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
        let engine = RuleEngine::new(&configs).unwrap();
        assert_eq!(engine.scan_value(input).len(), expected);
    }

    #[test]
    fn phone_builtin_through_engine() {
        let configs = vec![RuleConfig {
            rule_type: RuleType::Builtin,
            builtin: Some(BuiltinKind::Phone),
            ..cfg("phone")
        }];
        let engine = RuleEngine::new(&configs).unwrap();

        let m = engine.scan_value("call +44 20 7946 0958 now");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].rule.id, "phone");
        assert_eq!(m[0].matched_text, "+44 20 7946 0958");

        // bare digits should not match
        assert!(engine.scan_value("1234567890").is_empty());
    }
}
