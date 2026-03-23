use std::{fmt, path::PathBuf};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::pattern::PatternMatcher;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Allowlist {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub values: Vec<String>,
    #[serde(default)]
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Critical => write!(f, "CRITICAL"),
            Severity::High => write!(f, "HIGH"),
            Severity::Medium => write!(f, "MEDIUM"),
            Severity::Low => write!(f, "LOW"),
            Severity::Info => write!(f, "INFO"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Validator {
    Luhn,
    Ssn,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    #[default]
    Regex,
    Builtin,
    Script,
}

impl fmt::Display for RuleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuleType::Regex => write!(f, "regex"),
            RuleType::Builtin => write!(f, "builtin"),
            RuleType::Script => write!(f, "script"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinKind {
    CreditCard,
    Email,
    Iban,
    Phone,
    Ssn,
}

/// Per-rule scope filter (restrict which schema/table/column combinations a
/// rule applies to).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RuleScope {
    pub include_schemas: PatternMatcher,
    pub include_tables: PatternMatcher,
    pub exclude_tables: PatternMatcher,
    pub include_columns: PatternMatcher,
    pub exclude_columns: PatternMatcher,
}

impl RuleScope {
    pub fn validate(&self, rule_id: &str) -> Result<()> {
        for t in self.include_tables.raw() {
            if self.exclude_tables.raw().contains(t) {
                bail!("rule '{rule_id}': table '{t}' appears in both include_tables and exclude_tables");
            }
        }
        for c in self.include_columns.raw() {
            if self.exclude_columns.raw().contains(c) {
                bail!("rule '{rule_id}': column '{c}' appears in both include_columns and exclude_columns");
            }
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.include_schemas.is_empty()
            && self.include_tables.is_empty()
            && self.exclude_tables.is_empty()
            && self.include_columns.is_empty()
            && self.exclude_columns.is_empty()
    }

    pub fn matches(&self, schema: &str, table: &str, column: &str) -> bool {
        if !self.include_schemas.is_empty() && !self.include_schemas.is_match(schema) {
            return false;
        }
        if !self.include_tables.is_empty() && !self.include_tables.is_match(table) {
            return false;
        }
        if self.exclude_tables.is_match(table) {
            return false;
        }
        if !self.include_columns.is_empty() && !self.include_columns.is_match(column) {
            return false;
        }
        if self.exclude_columns.is_match(column) {
            return false;
        }
        true
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuleConfig {
    pub id: String,
    pub description: String,
    pub category: String,
    pub severity: Severity,
    #[serde(default, rename = "type")]
    pub rule_type: RuleType,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub validate: Option<Validator>,
    #[serde(default)]
    pub builtin: Option<BuiltinKind>,
    #[serde(default)]
    pub script: Option<PathBuf>,
    #[serde(default)]
    pub allowlist: Option<Allowlist>,
    #[serde(default)]
    pub scope: Option<RuleScope>,
    #[serde(default)]
    pub channels: Option<Vec<String>>,
}
