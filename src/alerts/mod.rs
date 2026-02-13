pub mod config;
pub mod dedup;
pub mod dispatcher;
pub mod jsonl;
pub mod log;
pub mod postgres;
pub mod slack;
pub mod stdout;
pub mod webhook;

use std::collections::BTreeMap;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use self::{jsonl::JsonlChannel, log::LogChannel, postgres::PostgresChannel, slack::SlackChannel, stdout::StdoutChannel, webhook::WebhookChannel};
use crate::scanner::Finding;

pub enum AlertChannel {
    Log(LogChannel),
    Stdout(StdoutChannel),
    Jsonl(JsonlChannel),
    Webhook(WebhookChannel),
    Slack(SlackChannel),
    Postgres(PostgresChannel),
    #[cfg(test)]
    Mock(testing::MockChannel),
}

impl AlertChannel {
    pub async fn send(&self, finding: &Finding) -> Result<()> {
        match self {
            AlertChannel::Log(ch) => ch.send(finding),
            AlertChannel::Stdout(ch) => ch.send(finding),
            AlertChannel::Jsonl(ch) => ch.send(finding),
            AlertChannel::Webhook(ch) => ch.send(finding).await,
            AlertChannel::Slack(ch) => ch.send(finding).await,
            AlertChannel::Postgres(ch) => ch.send(finding).await,
            #[cfg(test)]
            AlertChannel::Mock(ch) => ch.send(finding),
        }
    }

    pub async fn flush(&self) -> Result<()> {
        match self {
            AlertChannel::Slack(ch) => ch.flush().await,
            _ => Ok(()),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            AlertChannel::Log(_) => "log",
            AlertChannel::Stdout(_) => "stdout",
            AlertChannel::Jsonl(_) => "jsonl",
            AlertChannel::Webhook(_) => "webhook",
            AlertChannel::Slack(_) => "slack",
            AlertChannel::Postgres(_) => "postgres",
            #[cfg(test)]
            AlertChannel::Mock(_) => "mock",
        }
    }
}

/// JSON-serializable alert payload for webhook and stdout channels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertPayload {
    pub rule_id: String,
    pub description: String,
    pub category: String,
    pub severity: String,
    pub schema_name: String,
    pub table_name: String,
    pub column_name: String,
    pub masked_sample: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub primary_key: BTreeMap<String, String>,
    pub lsn: u64,
    pub timestamp: String,
}

impl From<&Finding> for AlertPayload {
    fn from(f: &Finding) -> Self {
        Self {
            rule_id: f.rule_id.clone(),
            description: f.description.clone(),
            category: f.category.clone(),
            severity: f.severity.to_string(),
            schema_name: f.schema_name.clone(),
            table_name: f.table_name.clone(),
            column_name: f.column_name.clone(),
            masked_sample: f.masked_sample.clone(),
            primary_key: f.primary_keys.iter().cloned().collect(),
            lsn: f.lsn,
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

#[cfg(test)]
pub mod testing {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use anyhow::{Result, bail};

    use crate::{rules::config::Severity, scanner::Finding};

    pub fn test_finding() -> Finding {
        Finding {
            rule_id: "test-rule".to_string(),
            description: "test description".to_string(),
            category: "test".to_string(),
            severity: Severity::High,
            schema_name: "public".to_string(),
            table_name: "events".to_string(),
            column_name: "data".to_string(),
            masked_sample: "***masked***".to_string(),
            value_hash: 0,
            primary_keys: vec![("id".to_string(), "1".to_string())],
            lsn: 1,
        }
    }

    pub struct MockChannel {
        call_count: Arc<AtomicUsize>,
        should_fail: bool,
    }

    impl MockChannel {
        pub fn new() -> (Self, Arc<AtomicUsize>) {
            let count = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    call_count: count.clone(),
                    should_fail: false,
                },
                count,
            )
        }

        pub fn failing() -> (Self, Arc<AtomicUsize>) {
            let count = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    call_count: count.clone(),
                    should_fail: true,
                },
                count,
            )
        }

        pub fn send(&self, _finding: &Finding) -> Result<()> {
            self.call_count.fetch_add(1, Ordering::Relaxed);
            if self.should_fail {
                bail!("mock channel failure");
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alerts::testing::test_finding;

    #[test]
    fn alert_payload_from_finding() {
        let payload = AlertPayload::from(&test_finding());
        assert_eq!(payload.rule_id, "test-rule");
        assert_eq!(payload.severity, "HIGH");
        assert_eq!(payload.lsn, 1);
        assert_eq!(payload.primary_key.get("id").unwrap(), "1");
        assert!(!payload.timestamp.is_empty());
    }

    #[test]
    fn alert_payload_serializes_to_json() {
        let payload = AlertPayload::from(&test_finding());
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("test-rule"));
        assert!(json.contains("***masked***"));
        assert!(json.contains("HIGH"));
        assert!(json.contains(r#""primary_key":{"id":"1"}"#));
    }
}
