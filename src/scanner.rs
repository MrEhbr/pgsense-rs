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

impl ScanFilter {
    pub fn matches_schema(&self, schema: &str) -> bool {
        self.include_schemas.is_empty() || self.include_schemas.iter().any(|s| s == schema)
    }

    pub fn matches_table(&self, table: &str) -> bool {
        !self.exclude_tables.iter().any(|t| t == table)
    }

    pub fn should_include_column(&self, column: &str) -> bool {
        !self.exclude_columns.iter().any(|c| c == column)
    }
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub database: String,
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
    pub channels: Option<Vec<String>>,
}

static HASHER: LazyLock<RandomState> = LazyLock::new(RandomState::new);

pub struct Scanner {
    engine: RuleEngine,
}

impl Scanner {
    pub fn new(engine: RuleEngine) -> Self {
        Self { engine }
    }

    pub fn scan(&self, event: &ScanEvent) -> Vec<Finding> {
        let mut findings = Vec::new();

        for col in &event.columns {
            if !is_scannable_type(&col.type_name) {
                continue;
            }

            let value = match &col.value {
                Some(v) => v,
                None => continue,
            };

            for m in self.engine.scan_value(value) {
                if let Some(scope) = &m.rule.scope
                    && !scope.matches(&event.schema_name, &event.table_name, &col.name)
                {
                    continue;
                }

                findings.push(Finding {
                    database: event.database.clone(),
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
                    channels: m.rule.channels.clone(),
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
                    .retain(|(col, _)| !matched_columns.contains(col.as_str()));
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
        rules::config::{RuleConfig, RuleScope, RuleType, Severity},
    };

    const MATCH: &str = "ALPHA-1";
    const NO_MATCH: &str = "clean";

    fn test_event(columns: Vec<ColumnValue>) -> ScanEvent {
        ScanEvent {
            database: "localhost/testdb".to_string(),
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
            scope: None,
            channels: None,
        }];
        let engine = RuleEngine::new(&rules).unwrap();
        Scanner::new(engine)
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
    #[case("schema_miss", ScanFilter { include_schemas: vec!["other".into()], ..Default::default() }, false)]
    #[case("schema_hit", ScanFilter { include_schemas: vec!["public".into()], ..Default::default() }, true)]
    #[case("schema_empty", ScanFilter::default(), true)]
    #[case("table_excluded", ScanFilter { exclude_tables: vec!["t1".into()], ..Default::default() }, false)]
    #[case("table_not_excluded", ScanFilter { exclude_tables: vec!["other".into()], ..Default::default() }, true)]
    fn filter_matches_event(#[case] _label: &str, #[case] filter: ScanFilter, #[case] expected: bool) {
        assert_eq!(filter.matches_schema("public") && filter.matches_table("t1"), expected);
    }

    #[test]
    fn filter_excludes_column() {
        let filter = ScanFilter {
            exclude_columns: vec!["c1".into()],
            ..Default::default()
        };
        assert!(!filter.should_include_column("c1"));
        assert!(filter.should_include_column("c2"));
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

    fn scanner_with_scope(scope: RuleScope) -> Scanner {
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
            scope: Some(scope),
            channels: None,
        }];
        let engine = RuleEngine::new(&rules).unwrap();
        Scanner::new(engine)
    }

    #[rstest::rstest]
    #[case("include_table_match", RuleScope { include_tables: vec!["t1".into()], ..Default::default() }, true)]
    #[case("include_table_miss", RuleScope { include_tables: vec!["other".into()], ..Default::default() }, false)]
    #[case("exclude_table_match", RuleScope { exclude_tables: vec!["t1".into()], ..Default::default() }, false)]
    #[case("exclude_table_miss", RuleScope { exclude_tables: vec!["other".into()], ..Default::default() }, true)]
    #[case("include_schema_match", RuleScope { include_schemas: vec!["public".into()], ..Default::default() }, true)]
    #[case("include_schema_miss", RuleScope { include_schemas: vec!["private".into()], ..Default::default() }, false)]
    #[case("include_column_match", RuleScope { include_columns: vec!["c1".into()], ..Default::default() }, true)]
    #[case("include_column_miss", RuleScope { include_columns: vec!["c2".into()], ..Default::default() }, false)]
    #[case("exclude_column_match", RuleScope { exclude_columns: vec!["c1".into()], ..Default::default() }, false)]
    #[case("exclude_column_miss", RuleScope { exclude_columns: vec!["other".into()], ..Default::default() }, true)]
    fn rule_scope_filtering(#[case] _label: &str, #[case] scope: RuleScope, #[case] expect_finding: bool) {
        let scanner = scanner_with_scope(scope);
        let event = test_event(vec![col("c1", Some(MATCH))]);
        let findings = scanner.scan(&event);
        assert_eq!(!findings.is_empty(), expect_finding, "expected finding={expect_finding}");
    }

    #[test]
    fn scope_empty_means_no_restriction() {
        let scanner = scanner_with_scope(RuleScope::default());
        let event = test_event(vec![col("c1", Some(MATCH))]);
        assert_eq!(scanner.scan(&event).len(), 1);
    }

    #[test]
    fn scope_combined_include_and_exclude() {
        let scope = RuleScope {
            include_tables: vec!["t1".into()],
            exclude_columns: vec!["c1".into()],
            ..Default::default()
        };
        let scanner = scanner_with_scope(scope);

        // c1 excluded by scope even though table matches
        let event = test_event(vec![col("c1", Some(MATCH))]);
        assert!(scanner.scan(&event).is_empty());

        // c2 not excluded, table matches
        let event = test_event(vec![col("c2", Some(MATCH))]);
        assert_eq!(scanner.scan(&event).len(), 1);
    }
}
