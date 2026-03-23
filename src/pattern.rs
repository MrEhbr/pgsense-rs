use std::fmt;

use anyhow::{Context, Result};
use globset::{Glob, GlobMatcher};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Deliberately narrow: only `*` and `?` are treated as glob metacharacters
/// because `[a-z]` and `{a,b}` can appear legitimately in PostgreSQL
/// identifiers.
fn is_glob_pattern(s: &str) -> bool {
    s.contains('*') || s.contains('?')
}

#[derive(Clone)]
enum Pattern {
    Exact(String),
    Glob(GlobMatcher),
}

impl Pattern {
    fn is_match(&self, value: &str) -> bool {
        match self {
            Pattern::Exact(s) => s == value,
            Pattern::Glob(m) => m.is_match(value),
        }
    }
}

/// A compiled list of patterns supporting both exact-string and glob matching.
///
/// Auto-detects format: strings containing `*` or `?` are compiled as globs;
/// everything else uses direct equality comparison.
///
/// Implements `Serialize`/`Deserialize` as `Vec<String>` — patterns are
/// compiled during deserialization, so invalid globs fail at config load time.
///
/// # Examples
///
/// ```
/// use pgsense_rs::pattern::PatternMatcher;
///
/// let m = PatternMatcher::compile(vec!["audit_*".into(), "tmp".into()]).unwrap();
/// assert!(m.is_match("audit_log"));
/// assert!(m.is_match("tmp"));
/// assert!(!m.is_match("user_audit"));
/// assert!(!m.is_match("temporary"));
/// ```
#[derive(Clone, Default)]
pub struct PatternMatcher {
    raw: Vec<String>,
    patterns: Vec<Pattern>,
}

impl PatternMatcher {
    /// Compile a list of pattern strings into a `PatternMatcher`.
    ///
    /// Strings containing `*` or `?` are compiled as glob patterns (via
    /// [`globset`]). All others are stored as exact-match strings.
    ///
    /// # Errors
    ///
    /// Returns an error if any glob pattern fails to compile.
    pub fn compile(raw: Vec<String>) -> Result<Self> {
        let patterns = raw
            .iter()
            .map(|s| {
                if is_glob_pattern(s) {
                    let matcher = Glob::new(s)
                        .with_context(|| format!("invalid glob pattern: {s}"))?
                        .compile_matcher();
                    Ok(Pattern::Glob(matcher))
                } else {
                    Ok(Pattern::Exact(s.clone()))
                }
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { raw, patterns })
    }

    /// Returns `true` if `value` matches any pattern in this matcher.
    pub fn is_match(&self, value: &str) -> bool {
        self.patterns.iter().any(|p| p.is_match(value))
    }

    /// Returns `true` if no patterns were compiled.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// The original pattern strings before compilation.
    pub fn raw(&self) -> &[String] {
        &self.raw
    }
}

impl fmt::Debug for PatternMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(&self.raw).finish()
    }
}

impl Serialize for PatternMatcher {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PatternMatcher {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let raw: Vec<String> = Vec::deserialize(deserializer)?;
        Self::compile(raw).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case("exact_hit", &["users"], "users", true)]
    #[case("exact_miss", &["users"], "orders", false)]
    #[case("exact_no_prefix", &["users"], "users_extra", false)]
    #[case("star_hit", &["audit_*"], "audit_log", true)]
    #[case("star_hit_2", &["audit_*"], "audit_trail", true)]
    #[case("star_miss_suffix", &["audit_*"], "user_audit", false)]
    #[case("star_miss_bare", &["audit_*"], "audit", false)]
    #[case("question_hit", &["db?"], "db1", true)]
    #[case("question_hit_2", &["db?"], "dba", true)]
    #[case("question_miss_long", &["db?"], "db10", false)]
    #[case("question_miss_short", &["db?"], "db", false)]
    #[case("mixed_exact_hit", &["exact_name", "tmp_*"], "exact_name", true)]
    #[case("mixed_glob_hit", &["exact_name", "tmp_*"], "tmp_work", true)]
    #[case("mixed_miss", &["exact_name", "tmp_*"], "other", false)]
    fn pattern_matching(#[case] _label: &str, #[case] patterns: &[&str], #[case] value: &str, #[case] expected: bool) {
        let owned: Vec<String> = patterns.iter().map(|s| s.to_string()).collect();
        let m = PatternMatcher::compile(owned).unwrap();
        assert_eq!(m.is_match(value), expected);
    }

    #[test]
    fn empty_matcher() {
        let m = PatternMatcher::default();
        assert!(m.is_empty());
        assert!(!m.is_match("anything"));
    }

    #[test]
    fn compile_error() {
        // "{unclosed*" contains a glob metachar (*) so it compiles as a glob,
        // but the unclosed alternate group makes globset reject it.
        let result = PatternMatcher::compile(vec!["{unclosed*".to_string()]);
        assert!(result.is_err(), "expected compile error for invalid glob");
    }

    #[test]
    fn raw_preserves_original_strings() {
        let m = PatternMatcher::compile(vec!["exact".into(), "glob_*".into()]).unwrap();
        assert_eq!(m.raw(), &["exact", "glob_*"]);
    }

    #[test]
    fn serde_roundtrip() {
        let original = PatternMatcher::compile(vec!["public".into(), "staging_*".into()]).unwrap();
        let json = serde_json::to_string(&original).unwrap();
        let restored: PatternMatcher = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.raw(), original.raw());
        assert!(restored.is_match("public"));
        assert!(restored.is_match("staging_v2"));
        assert!(!restored.is_match("private"));
    }
}
