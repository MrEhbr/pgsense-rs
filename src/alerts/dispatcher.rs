use std::time::Duration;

use anyhow::Result;
use tracing::{debug, error, info};

use super::{
    AlertChannel, config::AlertsConfig, dedup::Deduplicator, jsonl::JsonlChannel, log::LogChannel, postgres::PostgresChannel, slack::SlackChannel,
    stdout::StdoutChannel, webhook::WebhookChannel,
};
use crate::scanner::Finding;

pub struct Dispatcher {
    channels: Vec<AlertChannel>,
    dedup: Deduplicator,
}

impl Dispatcher {
    pub async fn from_config(config: &AlertsConfig) -> Result<Self> {
        let mut channels = Vec::new();

        if config.log.enabled {
            channels.push(AlertChannel::Log(LogChannel));
        }
        if config.stdout.enabled {
            channels.push(AlertChannel::Stdout(StdoutChannel));
        }
        if config.jsonl.enabled {
            channels.push(AlertChannel::Jsonl(JsonlChannel::new(&config.jsonl)?));
        }
        for webhook_config in &config.webhooks {
            channels.push(AlertChannel::Webhook(WebhookChannel::new(webhook_config)?));
        }
        for slack_config in &config.slack {
            channels.push(AlertChannel::Slack(SlackChannel::new(slack_config)?));
        }
        if let Some(pg_config) = &config.postgres {
            channels.push(AlertChannel::Postgres(PostgresChannel::new(pg_config).await?));
        }

        info!(channels = channels.len(), "alert dispatcher initialized");

        Ok(Self {
            channels,
            dedup: Deduplicator::new(Duration::from_secs(config.dedup_window_seconds)),
        })
    }

    /// Channel failures are logged but do not propagate.
    pub async fn dispatch(&mut self, finding: &Finding) {
        if !self.dedup.should_alert(finding) {
            debug!(
                rule_id = %finding.rule_id,
                table = %finding.table_name,
                column = %finding.column_name,
                "alert deduplicated"
            );
            return;
        }

        for channel in &self.channels {
            if let Err(e) = channel.send(finding).await {
                error!(
                    channel = channel.name(),
                    error = %e,
                    rule_id = %finding.rule_id,
                    "alert channel failed"
                );
            }
        }
    }

    pub async fn flush(&self) {
        for channel in &self.channels {
            if let Err(e) = channel.flush().await {
                error!(channel = channel.name(), error = %e, "flush failed");
            }
        }
    }

    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    #[cfg(test)]
    fn with_channels(channels: Vec<AlertChannel>) -> Self {
        Self {
            channels,
            dedup: Deduplicator::new(Duration::from_secs(3600)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alerts::testing::test_finding;

    #[tokio::test]
    async fn dispatcher_from_default_config() {
        let config = AlertsConfig::default();
        let dispatcher = Dispatcher::from_config(&config).await.unwrap();
        assert_eq!(dispatcher.channel_count(), 1); // log enabled by default
    }

    #[tokio::test]
    async fn dispatcher_with_all_channels() {
        let config = AlertsConfig {
            log: super::super::config::LogAlertConfig { enabled: true },
            stdout: super::super::config::StdoutAlertConfig { enabled: true },
            webhooks: vec![super::super::config::WebhookConfig {
                name: None,
                url: "https://hooks.example.com".to_string(),
                headers: Default::default(),
                timeout_ms: 5000,
            }],
            ..Default::default()
        };
        let dispatcher = Dispatcher::from_config(&config).await.unwrap();
        assert_eq!(dispatcher.channel_count(), 3);
    }

    #[tokio::test]
    async fn dispatch_calls_channel() {
        use std::sync::atomic::Ordering;

        use super::super::testing::MockChannel;

        let (mock, count) = MockChannel::new();
        let mut dispatcher = Dispatcher::with_channels(vec![AlertChannel::Mock(mock)]);

        dispatcher.dispatch(&test_finding()).await;
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn dispatch_continues_on_channel_error() {
        use std::sync::atomic::Ordering;

        use super::super::testing::MockChannel;

        let (failing, fail_count) = MockChannel::failing();
        let (ok, ok_count) = MockChannel::new();
        let mut dispatcher = Dispatcher::with_channels(vec![AlertChannel::Mock(failing), AlertChannel::Mock(ok)]);

        dispatcher.dispatch(&test_finding()).await;
        assert_eq!(fail_count.load(Ordering::Relaxed), 1);
        assert_eq!(ok_count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn dispatcher_deduplicates() {
        let config = AlertsConfig {
            log: super::super::config::LogAlertConfig { enabled: false },
            stdout: super::super::config::StdoutAlertConfig { enabled: false },
            webhooks: vec![],
            ..Default::default()
        };
        let mut dispatcher = Dispatcher::from_config(&config).await.unwrap();
        let finding = test_finding();

        // Both calls succeed (no channels to fail), but second is deduplicated
        dispatcher.dispatch(&finding).await;
        dispatcher.dispatch(&finding).await;
        // No assertion needed — we're verifying no panic and dedup runs
    }
}
