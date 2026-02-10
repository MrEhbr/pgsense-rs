use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::scanner::Finding;

const PRUNE_THRESHOLD: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DedupKey {
    schema_name: String,
    table_name: String,
    column_name: String,
    rule_id: String,
    value_hash: u64,
}

/// Suppresses duplicate alerts for the same (schema, table, column, rule,
/// value) within a time window. Uses a hash of the original matched text
/// so different values always produce distinct keys regardless of masking.
pub struct Deduplicator {
    seen: HashMap<DedupKey, Instant>,
    window: Duration,
}

impl Deduplicator {
    pub fn new(window: Duration) -> Self {
        Self { seen: HashMap::new(), window }
    }

    pub fn should_alert(&mut self, finding: &Finding) -> bool {
        if self.seen.len() > PRUNE_THRESHOLD {
            let now = Instant::now();
            self.seen
                .retain(|_, last| now.duration_since(*last) < self.window);
        }

        let key = DedupKey {
            schema_name: finding.schema_name.clone(),
            table_name: finding.table_name.clone(),
            column_name: finding.column_name.clone(),
            rule_id: finding.rule_id.clone(),
            value_hash: finding.value_hash,
        };
        let now = Instant::now();

        match self.seen.get(&key) {
            Some(last) if now.duration_since(*last) < self.window => false,
            _ => {
                self.seen.insert(key, now);
                true
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::rules::config::Severity;

    fn test_finding(schema: &str, table: &str, column: &str, rule_id: &str, value: &str) -> Finding {
        use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};
        Finding {
            rule_id: rule_id.to_string(),
            description: "test".to_string(),
            category: "TEST".to_string(),
            severity: Severity::High,
            schema_name: schema.to_string(),
            table_name: table.to_string(),
            column_name: column.to_string(),
            masked_sample: "****".to_string(),
            value_hash: BuildHasherDefault::<DefaultHasher>::new().hash_one(value),
            primary_keys: vec![],
            lsn: 100,
        }
    }

    #[test]
    fn duplicate_suppressed_within_window() {
        let mut dedup = Deduplicator::new(Duration::from_secs(300));
        let finding = test_finding("public", "data", "col1", "rule1", "foo");

        assert!(dedup.should_alert(&finding));
        assert!(!dedup.should_alert(&finding));
        assert!(!dedup.should_alert(&finding));
    }

    #[rstest]
    #[case::different_columns(
        ("public", "data", "col1", "rule1", "foo"),
        ("public", "data", "col2", "rule1", "foo"),
    )]
    #[case::different_rules(
        ("public", "data", "col1", "rule1", "foo"),
        ("public", "data", "col1", "rule2", "foo"),
    )]
    #[case::different_values(
        ("public", "data", "col1", "rule1", "foo"),
        ("public", "data", "col1", "rule1", "bar"),
    )]
    #[case::different_schemas(
        ("public",  "data", "col1", "rule1", "foo"),
        ("private", "data", "col1", "rule1", "foo"),
    )]
    fn distinct_keys_not_suppressed(#[case] a: (&str, &str, &str, &str, &str), #[case] b: (&str, &str, &str, &str, &str)) {
        let mut dedup = Deduplicator::new(Duration::from_secs(300));

        assert!(dedup.should_alert(&test_finding(a.0, a.1, a.2, a.3, a.4)));
        assert!(dedup.should_alert(&test_finding(b.0, b.1, b.2, b.3, b.4)));
    }

    #[test]
    fn expired_entries_allow_re_alert() {
        let mut dedup = Deduplicator::new(Duration::from_millis(1));
        let finding = test_finding("public", "data", "col1", "rule1", "foo");

        assert!(dedup.should_alert(&finding));
        std::thread::sleep(Duration::from_millis(5));
        assert!(dedup.should_alert(&finding));
    }
}
