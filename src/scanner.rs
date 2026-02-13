use std::{collections::hash_map::RandomState, hash::BuildHasher, sync::LazyLock};

use serde::{Deserialize, Serialize};

use crate::{
    events::{ScanEvent, is_scannable_type},
    rules::{config::Severity, engine::RuleEngine, masking},
};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ScanFilter {
    #[serde(default)]
    pub include_schemas: Vec<String>,
    #[serde(default)]
    pub exclude_tables: Vec<String>,
    #[serde(default)]
    pub exclude_columns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub rule_id: String,
    pub description: String,
    pub category: String,
    pub severity: Severity,
    pub schema_name: String,
    pub table_name: String,
    pub column_name: String,
    pub masked_sample: String,
    /// Hash of the original matched text for dedup without retaining sensitive
    /// data.
    pub value_hash: u64,
    pub primary_keys: Vec<(String, String)>,
    pub lsn: u64,
}

static HASHER: LazyLock<RandomState> = LazyLock::new(RandomState::new);

pub struct Scanner {
    engine: RuleEngine,
    filter: ScanFilter,
}

impl Scanner {
    pub fn new(engine: RuleEngine, filter: ScanFilter) -> Self {
        Self { engine, filter }
    }

    pub fn scan(&self, event: &ScanEvent) -> Vec<Finding> {
        if !self.filter.include_schemas.is_empty() && !self.filter.include_schemas.contains(&event.schema_name) {
            return Vec::new();
        }

        if self.filter.exclude_tables.contains(&event.table_name) {
            return Vec::new();
        }

        let mut findings = Vec::new();

        for col in &event.columns {
            if self.filter.exclude_columns.contains(&col.name) {
                continue;
            }

            if !is_scannable_type(&col.type_name) {
                continue;
            }

            let value = match &col.value {
                Some(v) => v,
                None => continue,
            };

            for m in self.engine.scan_value(value) {
                findings.push(Finding {
                    rule_id: m.rule.id.clone(),
                    description: m.rule.description.clone(),
                    category: m.rule.category.clone(),
                    severity: m.rule.severity,
                    schema_name: event.schema_name.clone(),
                    table_name: event.table_name.clone(),
                    column_name: col.name.clone(),
                    masked_sample: masking::mask(&m.matched_text),
                    value_hash: HASHER.hash_one(&m.matched_text),
                    primary_keys: event.primary_keys.clone(),
                    lsn: event.commit_lsn,
                });
            }
        }

        // Strip matched columns from primary_keys so sensitive values
        // don't leak in alert output (especially with REPLICA IDENTITY FULL).
        if !findings.is_empty() {
            let matched_columns: std::collections::HashSet<String> = findings.iter().map(|f| f.column_name.clone()).collect();
            for finding in &mut findings {
                finding
                    .primary_keys
                    .retain(|(col, _)| !matched_columns.contains(col));
            }
        }

        findings
    }

    pub fn rule_count(&self) -> usize {
        self.engine.rule_count()
    }
}

#[cfg(test)]
mod tests {
    use etl::types::TableId;

    use super::*;
    use crate::{
        events::{Action, ColumnValue, ScanEvent},
        rules::config::{RuleConfig, RuleType, Severity},
    };

    const MATCH: &str = "ALPHA-1";
    const NO_MATCH: &str = "clean";

    fn test_event(columns: Vec<ColumnValue>) -> ScanEvent {
        ScanEvent {
            table_id: TableId(1),
            schema_name: "public".to_string(),
            table_name: "t1".to_string(),
            action: Action::Insert,
            columns,
            primary_keys: vec![("id".to_string(), "1".to_string())],
            start_lsn: 100,
            commit_lsn: 200,
        }
    }

    fn col(name: &str, value: Option<&str>) -> ColumnValue {
        ColumnValue {
            name: name.to_string(),
            type_name: "text".to_string(),
            value: value.map(|s| s.to_string()),
        }
    }

    fn typed_col(name: &str, type_name: &str, value: Option<&str>) -> ColumnValue {
        ColumnValue {
            name: name.to_string(),
            type_name: type_name.to_string(),
            value: value.map(|s| s.to_string()),
        }
    }

    fn scanner_with_defaults() -> Scanner {
        let rules = vec![RuleConfig {
            id: "rule-alpha".into(),
            description: "Matches ALPHA marker".into(),
            category: "CAT_A".into(),
            severity: Severity::Critical,
            rule_type: RuleType::Regex,
            pattern: Some(r"ALPHA-\d+".into()),
            validate: None,
            builtin: None,
            script: None,
            allowlist: None,
        }];
        let engine = RuleEngine::new(&rules).unwrap();
        Scanner::new(engine, ScanFilter::default())
    }

    #[test]
    fn detects_matching_value() {
        let scanner = scanner_with_defaults();
        let event = test_event(vec![col("c1", Some(MATCH))]);

        let findings = scanner.scan(&event);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "rule-alpha");
        assert_eq!(findings[0].severity, Severity::Critical);
        assert_eq!(findings[0].column_name, "c1");
    }

    #[rstest::rstest]
    #[case("null_column", vec![col("c1", None)])]
    #[case("clean_data", vec![col("c1", Some(NO_MATCH)), col("c2", Some(NO_MATCH))])]
    #[case("non_text_types", vec![typed_col("c1", "int4", Some(MATCH)), typed_col("c2", "bool", Some(MATCH))])]
    fn no_findings(#[case] _label: &str, #[case] columns: Vec<ColumnValue>) {
        let scanner = scanner_with_defaults();
        let event = test_event(columns);
        assert!(scanner.scan(&event).is_empty());
    }

    #[rstest::rstest]
    #[case("schema", ScanFilter { include_schemas: vec!["other".into()], ..Default::default() })]
    #[case("table", ScanFilter { exclude_tables: vec!["t1".into()], ..Default::default() })]
    #[case("column", ScanFilter { exclude_columns: vec!["c1".into()], ..Default::default() })]
    fn filter_excludes(#[case] _label: &str, #[case] filter: ScanFilter) {
        let rules = vec![RuleConfig {
            id: "rule-alpha".into(),
            description: "Matches ALPHA marker".into(),
            category: "CAT_A".into(),
            severity: Severity::Critical,
            rule_type: RuleType::Regex,
            pattern: Some(r"ALPHA-\d+".into()),
            validate: None,
            builtin: None,
            script: None,
            allowlist: None,
        }];
        let engine = RuleEngine::new(&rules).unwrap();
        let scanner = Scanner::new(engine, filter);

        let event = test_event(vec![col("c1", Some(MATCH))]);
        assert!(scanner.scan(&event).is_empty());
    }

    #[test]
    fn matched_columns_stripped_from_primary_keys() {
        let scanner = scanner_with_defaults();
        let event = ScanEvent {
            primary_keys: vec![
                ("id".to_string(), "1".to_string()),
                ("c1".to_string(), MATCH.to_string()),
                ("c2".to_string(), "safe".to_string()),
            ],
            ..test_event(vec![col("c1", Some(MATCH))])
        };

        let findings = scanner.scan(&event);
        assert_eq!(findings.len(), 1);
        let pk_cols: Vec<&str> = findings[0]
            .primary_keys
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert!(pk_cols.contains(&"id"));
        assert!(pk_cols.contains(&"c2"));
        assert!(!pk_cols.contains(&"c1"));
    }

    #[test]
    fn non_matched_primary_keys_preserved() {
        let scanner = scanner_with_defaults();
        let event = test_event(vec![col("c1", Some(MATCH))]);

        let findings = scanner.scan(&event);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].primary_keys, vec![("id".to_string(), "1".to_string())]);
    }
}
