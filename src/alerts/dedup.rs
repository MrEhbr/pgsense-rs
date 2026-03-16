use std::{
    collections::{HashMap, hash_map::Entry},
    hash::{BuildHasher, BuildHasherDefault, DefaultHasher, Hash, Hasher},
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::scanner::Finding;

const PRUNE_THRESHOLD: usize = 10_000;

/// Suppresses duplicate alerts for the same (schema, table, column, rule,
/// value) within a time window. Uses a compound hash of all key fields to
/// avoid cloning strings on every lookup.
pub struct Deduplicator {
    seen: Mutex<HashMap<u64, Instant>>,
    hasher: BuildHasherDefault<DefaultHasher>,
    window: Duration,
}

impl Deduplicator {
    pub fn new(window: Duration) -> Self {
        Self {
            seen: Mutex::new(HashMap::new()),
            hasher: BuildHasherDefault::default(),
            window,
        }
    }

    pub fn should_alert(&self, finding: &Finding) -> bool {
        let mut seen = self.seen.lock().unwrap();

        if seen.len() > PRUNE_THRESHOLD {
            let now = Instant::now();
            seen.retain(|_, last| now.duration_since(*last) < self.window);
        }

        let key = self.dedup_key(finding);
        let now = Instant::now();

        match seen.entry(key) {
            Entry::Occupied(e) if now.duration_since(*e.get()) < self.window => false,
            Entry::Occupied(mut e) => {
                e.insert(now);
                true
            },
            Entry::Vacant(e) => {
                e.insert(now);
                true
            },
        }
    }

    fn dedup_key(&self, finding: &Finding) -> u64 {
        let mut hasher = self.hasher.build_hasher();
        finding.database.hash(&mut hasher);
        finding.schema_name.hash(&mut hasher);
        finding.table_name.hash(&mut hasher);
        finding.column_name.hash(&mut hasher);
        finding.rule_id.hash(&mut hasher);
        finding.value_hash.hash(&mut hasher);
        hasher.finish()
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
            database: "localhost/testdb".to_string(),
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
            channels: None,
        }
    }

    #[test]
    fn duplicate_suppressed_within_window() {
        let dedup = Deduplicator::new(Duration::from_secs(300));
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
        let dedup = Deduplicator::new(Duration::from_secs(300));

        assert!(dedup.should_alert(&test_finding(a.0, a.1, a.2, a.3, a.4)));
        assert!(dedup.should_alert(&test_finding(b.0, b.1, b.2, b.3, b.4)));
    }

    #[test]
    fn expired_entries_allow_re_alert() {
        let dedup = Deduplicator::new(Duration::from_millis(1));
        let finding = test_finding("public", "data", "col1", "rule1", "foo");

        assert!(dedup.should_alert(&finding));
        std::thread::sleep(Duration::from_millis(5));
        assert!(dedup.should_alert(&finding));
    }
}
