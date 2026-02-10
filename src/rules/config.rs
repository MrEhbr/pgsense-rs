use std::{fmt, path::PathBuf};

use serde::{Deserialize, Serialize};

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
    Ssn,
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
}
