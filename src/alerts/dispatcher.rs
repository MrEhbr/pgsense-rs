use std::time::Duration;

use anyhow::Result;
use tracing::{debug, error, info, warn};

use super::{
    AlertChannel, config::AlertsConfig, dedup::Deduplicator, jsonl::JsonlChannel, log::LogChannel, postgres::PostgresChannel, slack::SlackChannel,
    stdout::StdoutChannel, webhook::WebhookChannel,
};
use crate::scanner::Finding;

struct NamedChannel {
    name: String,
    channel: AlertChannel,
}

pub struct Dispatcher {
    channels: Vec<NamedChannel>,
    dedup: Deduplicator,
}

impl Dispatcher {
    pub async fn from_config(config: &AlertsConfig) -> Result<Self> {
        let mut channels = Vec::new();

        if config.log.enabled {
            channels.push(NamedChannel {
                name: "log".into(),
                channel: AlertChannel::Log(LogChannel),
            });
        }
        if config.stdout.enabled {
            channels.push(NamedChannel {
                name: "stdout".into(),
                channel: AlertChannel::Stdout(StdoutChannel),
            });
        }
        if config.jsonl.enabled {
            let name = config.jsonl.name.clone().unwrap_or_else(|| "jsonl".into());
            channels.push(NamedChannel {
                name,
                channel: AlertChannel::Jsonl(JsonlChannel::new(&config.jsonl)?),
            });
        }
        for (i, webhook_config) in config.webhooks.iter().enumerate() {
            let name = webhook_config.name.clone().unwrap_or_else(|| {
                if config.webhooks.len() == 1 {
                    "webhook".into()
                } else {
                    format!("webhook-{}", i + 1)
                }
            });
            channels.push(NamedChannel {
                name,
                channel: AlertChannel::Webhook(WebhookChannel::new(webhook_config)?),
            });
        }
        for (i, slack_config) in config.slack.iter().enumerate() {
            let name = slack_config.name.clone().unwrap_or_else(|| {
                if config.slack.len() == 1 {
                    "slack".into()
                } else {
                    format!("slack-{}", i + 1)
                }
            });
            channels.push(NamedChannel {
                name,
                channel: AlertChannel::Slack(SlackChannel::new(slack_config)?),
            });
        }
        if let Some(pg_config) = &config.postgres {
            let name = pg_config.name.clone().unwrap_or_else(|| "postgres".into());
            channels.push(NamedChannel {
                name,
                channel: AlertChannel::Postgres(PostgresChannel::new(pg_config).await?),
            });
        }

        let mut seen = std::collections::HashSet::new();
        for nc in &channels {
            if !seen.insert(&nc.name) {
                warn!(name = %nc.name, "duplicate alert channel name — routing may be ambiguous");
            }
        }

        info!(channels = channels.len(), "alert dispatcher initialized");

        Ok(Self {
            channels,
            dedup: Deduplicator::new(Duration::from_secs(config.dedup_window_seconds)),
        })
    }

    /// Channel failures are logged but do not propagate. When the finding
    /// specifies `channels`, only the named channels are called; `None`
    /// fans out to all channels (backward compatible).
    pub async fn dispatch(&self, finding: &Finding) {
        if !self.dedup.should_alert(finding) {
            debug!(
                rule_id = %finding.rule_id,
                table = %finding.table_name,
                column = %finding.column_name,
                "alert deduplicated"
            );
            return;
        }

        for nc in &self.channels {
            if let Some(ref allowed) = finding.channels
                && !allowed.iter().any(|a| a == &nc.name)
            {
                continue;
            }
            match nc.channel.send(finding).await {
                Ok(()) => {
                    metrics::counter!(crate::metrics::ALERTS_TOTAL, "channel" => nc.name.clone(), "status" => "ok").increment(1);
                },
                Err(e) => {
                    metrics::counter!(crate::metrics::ALERTS_TOTAL, "channel" => nc.name.clone(), "status" => "error").increment(1);
                    error!(
                        channel = %nc.name,
                        error = %e,
                        rule_id = %finding.rule_id,
                        "alert channel failed"
                    );
                },
            }
        }
    }

    pub async fn flush(&self) {
        for nc in &self.channels {
            if let Err(e) = nc.channel.flush().await {
                error!(channel = %nc.name, error = %e, "flush failed");
            }
        }
    }

    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    pub fn channel_names(&self) -> Vec<&str> {
        self.channels.iter().map(|nc| nc.name.as_str()).collect()
    }

    pub fn validate_channel_routing(&self, rules: &[crate::rules::config::RuleConfig]) {
        let known: std::collections::HashSet<&str> = self.channel_names().into_iter().collect();
        for rule in rules {
            if let Some(channels) = &rule.channels {
                for ch in channels {
                    if !known.contains(ch.as_str()) {
                        tracing::warn!(rule_id = %rule.id, channel = %ch, "rule references unknown alert channel");
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub fn default_for_test() -> Self {
        Self {
            channels: Vec::new(),
            dedup: Deduplicator::new(Duration::from_secs(3600)),
        }
    }

    #[cfg(test)]
    fn with_named_channels(channels: Vec<(String, AlertChannel)>) -> Self {
        Self {
            channels: channels
                .into_iter()
                .map(|(name, channel)| NamedChannel { name, channel })
                .collect(),
            dedup: Deduplicator::new(Duration::from_secs(3600)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::alerts::testing::{MockChannel, test_finding};

    fn mock(name: &str) -> (String, AlertChannel, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        let (m, count) = MockChannel::new();
        (name.to_string(), AlertChannel::Mock(m), count)
    }

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
        let (name, ch, count) = mock("ch-a");
        let dispatcher = Dispatcher::with_named_channels(vec![(name, ch)]);

        dispatcher.dispatch(&test_finding()).await;
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn dispatch_continues_on_channel_error() {
        let (failing, fail_count) = MockChannel::failing();
        let (ok, ok_count) = MockChannel::new();
        let dispatcher = Dispatcher::with_named_channels(vec![
            ("fail".into(), AlertChannel::Mock(failing)),
            ("ok".into(), AlertChannel::Mock(ok)),
        ]);

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
        let dispatcher = Dispatcher::from_config(&config).await.unwrap();
        let finding = test_finding();

        // Both calls succeed (no channels to fail), but second is deduplicated
        dispatcher.dispatch(&finding).await;
        dispatcher.dispatch(&finding).await;
    }

    #[tokio::test]
    async fn from_config_assigns_default_names() {
        let config = AlertsConfig {
            log: super::super::config::LogAlertConfig { enabled: true },
            stdout: super::super::config::StdoutAlertConfig { enabled: true },
            ..Default::default()
        };
        let dispatcher = Dispatcher::from_config(&config).await.unwrap();
        let names = dispatcher.channel_names();
        assert!(names.contains(&"log"));
        assert!(names.contains(&"stdout"));
    }

    #[tokio::test]
    async fn dispatch_routes_to_specified_channels() {
        let (name_a, ch_a, count_a) = mock("ch-a");
        let (name_b, ch_b, count_b) = mock("ch-b");
        let dispatcher = Dispatcher::with_named_channels(vec![(name_a, ch_a), (name_b, ch_b)]);

        let mut finding = test_finding();
        finding.channels = Some(vec!["ch-a".into()]);

        dispatcher.dispatch(&finding).await;
        assert_eq!(count_a.load(Ordering::Relaxed), 1);
        assert_eq!(count_b.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn dispatch_routes_to_all_when_channels_is_none() {
        let (name_a, ch_a, count_a) = mock("ch-a");
        let (name_b, ch_b, count_b) = mock("ch-b");
        let dispatcher = Dispatcher::with_named_channels(vec![(name_a, ch_a), (name_b, ch_b)]);

        dispatcher.dispatch(&test_finding()).await;
        assert_eq!(count_a.load(Ordering::Relaxed), 1);
        assert_eq!(count_b.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn dispatch_routes_to_multiple_specified_channels() {
        let (name_a, ch_a, count_a) = mock("ch-a");
        let (name_b, ch_b, count_b) = mock("ch-b");
        let (name_c, ch_c, count_c) = mock("ch-c");
        let dispatcher = Dispatcher::with_named_channels(vec![(name_a, ch_a), (name_b, ch_b), (name_c, ch_c)]);

        let mut finding = test_finding();
        finding.channels = Some(vec!["ch-a".into(), "ch-c".into()]);

        dispatcher.dispatch(&finding).await;
        assert_eq!(count_a.load(Ordering::Relaxed), 1);
        assert_eq!(count_b.load(Ordering::Relaxed), 0);
        assert_eq!(count_c.load(Ordering::Relaxed), 1);
    }
}
