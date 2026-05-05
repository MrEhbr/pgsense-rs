use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use secrecy::ExposeSecret;
use serde_json::json;
use tokio::{sync::Mutex, task::JoinHandle, time::MissedTickBehavior};
use tracing::warn;

use super::config::SlackConfig;
use crate::{rules::config::Severity, scanner::Finding};

const SLACK_API_URL: &str = "https://slack.com/api/chat.postMessage";

struct BufferState {
    findings: Vec<Finding>,
    first_buffered_at: Option<Instant>,
}

struct Inner {
    client: reqwest::Client,
    api_url: String,
    channel: String,
    username: Option<String>,
    icon_emoji: Option<String>,
    buffer: Mutex<BufferState>,
    batch_size: usize,
    batch_window: Duration,
    max_retries: u32,
}

pub struct SlackChannel {
    inner: Arc<Inner>,
    _flush_task: JoinHandle<()>,
}

impl Drop for SlackChannel {
    fn drop(&mut self) {
        self._flush_task.abort();
    }
}

impl SlackChannel {
    pub fn new(config: &SlackConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                let auth = format!("Bearer {}", config.token.expose().expose_secret())
                    .parse::<reqwest::header::HeaderValue>()
                    .context("invalid slack token characters")?;
                headers.insert(reqwest::header::AUTHORIZATION, auth);
                headers
            })
            .build()
            .context("failed to build Slack HTTP client")?;

        let inner = Arc::new(Inner {
            client,
            api_url: SLACK_API_URL.to_string(),
            channel: config.channel.clone(),
            username: config.username.clone(),
            icon_emoji: config.icon_emoji.clone(),
            buffer: Mutex::new(BufferState {
                findings: Vec::new(),
                first_buffered_at: None,
            }),
            batch_size: config.batch_size,
            batch_window: Duration::from_millis(config.batch_window_ms),
            max_retries: config.max_retries,
        });

        let task_inner = inner.clone();
        let flush_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                if let Err(e) = task_inner.flush(false).await {
                    warn!(error = %e, "slack batch flush failed");
                }
            }
        });

        Ok(Self {
            inner,
            _flush_task: flush_task,
        })
    }

    pub async fn send(&self, finding: &Finding) -> Result<()> {
        self.inner.send(finding).await
    }

    pub async fn flush(&self) -> Result<()> {
        self.inner.flush(true).await
    }
}

impl SlackConfig {
    pub async fn auth_test(&self, client: &reqwest::Client) -> Result<(), String> {
        let resp = client
            .post("https://slack.com/api/auth.test")
            .bearer_auth(self.token.expose().expose_secret())
            .send()
            .await
            .map_err(|e| format!("request failed — {e}"))?;
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("invalid response — {e}"))?;
        if body.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            Ok(())
        } else {
            let err = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            Err(format!("auth.test rejected — {err}"))
        }
    }
}

impl Inner {
    async fn send(&self, finding: &Finding) -> Result<()> {
        let full = {
            let mut buf = self.buffer.lock().await;
            buf.findings.push(finding.clone());
            if buf.first_buffered_at.is_none() {
                buf.first_buffered_at = Some(Instant::now());
            }
            buf.findings.len() >= self.batch_size
        };

        if full {
            self.flush(true).await?;
        }

        Ok(())
    }

    async fn flush(&self, force: bool) -> Result<()> {
        let batch = {
            let mut buf = self.buffer.lock().await;
            let should_drain = if force {
                !buf.findings.is_empty()
            } else {
                buf.first_buffered_at
                    .is_some_and(|t| t.elapsed() >= self.batch_window)
            };
            if !should_drain {
                return Ok(());
            }
            buf.first_buffered_at = None;
            std::mem::take(&mut buf.findings)
        };

        self.send_batch(&batch).await
    }

    async fn send_batch(&self, findings: &[Finding]) -> Result<()> {
        let payload = self.build_payload(findings);

        for attempt in 0..=self.max_retries {
            let response = self
                .client
                .post(&self.api_url)
                .json(&payload)
                .send()
                .await
                .context("Slack API request failed")?;

            let status = response.status();

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                if attempt == self.max_retries {
                    anyhow::bail!("Slack API rate-limited after {} retries", self.max_retries);
                }

                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);

                let backoff = 1u64 << attempt;
                let delay = retry_after.max(backoff);

                warn!(
                    attempt = attempt + 1,
                    max_retries = self.max_retries,
                    retry_after_secs = delay,
                    "Slack API rate-limited, retrying"
                );

                tokio::time::sleep(Duration::from_secs(delay)).await;
                continue;
            }

            let body: serde_json::Value = response
                .json()
                .await
                .context("failed to parse Slack API response")?;

            if !status.is_success() {
                anyhow::bail!("Slack API returned HTTP {status}");
            }

            if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
                let error = body
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                anyhow::bail!("Slack API error: {error}");
            }

            return Ok(());
        }

        unreachable!()
    }

    fn build_payload(&self, findings: &[Finding]) -> serde_json::Value {
        if findings.len() == 1 {
            return self.single_payload(&findings[0]);
        }

        let highest_severity = findings
            .iter()
            .map(|f| &f.severity)
            .min_by_key(|s| severity_rank(s))
            .unwrap();

        let color = severity_color(highest_severity);
        let emoji = severity_emoji(highest_severity);

        let mut blocks: Vec<serde_json::Value> = Vec::new();

        blocks.push(json!({
            "type": "header",
            "text": {
                "type": "plain_text",
                "text": format!("{emoji} {} Sensitive Data Findings", findings.len()),
            }
        }));

        for (i, finding) in findings.iter().enumerate() {
            if i > 0 {
                blocks.push(json!({ "type": "divider" }));
            }

            let femoji = severity_emoji(&finding.severity);
            let fseverity = finding.severity.to_string();

            blocks.push(json!({
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": format!(
                        "{femoji} *{fseverity}* — {}\nSample: `{}`",
                        finding.description, finding.masked_sample,
                    ),
                }
            }));

            blocks.push(json!({
                "type": "context",
                "elements": [{
                    "type": "mrkdwn",
                    "text": format!(
                        "`{}` · `{}.{}.{}` · Rule: *{}* · Category: *{}* · LSN: `{}`",
                        finding.database, finding.schema_name, finding.table_name, finding.column_name,
                        finding.rule_id, finding.category, finding.lsn,
                    ),
                }]
            }));
        }

        let mut payload = json!({
            "channel": self.channel,
            "attachments": [{
                "color": color,
                "blocks": blocks,
            }]
        });

        if let Some(username) = &self.username {
            payload["username"] = json!(username);
        }
        if let Some(icon) = &self.icon_emoji {
            payload["icon_emoji"] = json!(icon);
        }

        payload
    }

    fn single_payload(&self, finding: &Finding) -> serde_json::Value {
        let color = severity_color(&finding.severity);
        let emoji = severity_emoji(&finding.severity);
        let severity = finding.severity.to_string();

        let mut payload = json!({
            "channel": self.channel,
            "attachments": [{
                "color": color,
                "blocks": [
                    {
                        "type": "section",
                        "text": {
                            "type": "mrkdwn",
                            "text": format!("{emoji} *{severity}* — {}", finding.description),
                        }
                    },
                    {
                        "type": "section",
                        "fields": [
                            { "type": "mrkdwn", "text": format!("*Rule*\n{}", finding.rule_id) },
                            { "type": "mrkdwn", "text": format!("*Category*\n{}", finding.category) },
                            { "type": "mrkdwn", "text": format!("*Database*\n{}", finding.database) },
                            { "type": "mrkdwn", "text": format!("*Table*\n{}.{}", finding.schema_name, finding.table_name) },
                            { "type": "mrkdwn", "text": format!("*Column*\n{}", finding.column_name) },
                        ]
                    },
                    {
                        "type": "section",
                        "text": {
                            "type": "mrkdwn",
                            "text": format!("*Sample*\n`{}`", finding.masked_sample),
                        }
                    },
                    {
                        "type": "context",
                        "elements": [{
                            "type": "mrkdwn",
                            "text": format!("LSN: {}", finding.lsn),
                        }]
                    }
                ]
            }]
        });

        if let Some(username) = &self.username {
            payload["username"] = json!(username);
        }
        if let Some(icon) = &self.icon_emoji {
            payload["icon_emoji"] = json!(icon);
        }

        payload
    }
}

fn severity_rank(severity: &Severity) -> u8 {
    match severity {
        Severity::Critical => 0,
        Severity::High => 1,
        Severity::Medium => 2,
        Severity::Low => 3,
        Severity::Info => 4,
    }
}

fn severity_color(severity: &Severity) -> &'static str {
    match severity {
        Severity::Critical => "#d32f2f",
        Severity::High => "#e65100",
        Severity::Medium => "#f9a825",
        Severity::Low => "#1565c0",
        Severity::Info => "#757575",
    }
}

fn severity_emoji(severity: &Severity) -> &'static str {
    match severity {
        Severity::Critical => "\u{1F534}",
        Severity::High => "\u{1F7E0}",
        Severity::Medium => "\u{1F7E1}",
        Severity::Low => "\u{1F535}",
        Severity::Info => "\u{26AA}",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alerts::testing::test_finding;

    fn test_channel() -> SlackChannel {
        let config = test_config();
        let inner = Arc::new(Inner {
            client: reqwest::Client::new(),
            api_url: "http://127.0.0.1:1".to_string(),
            channel: config.channel,
            username: config.username,
            icon_emoji: config.icon_emoji,
            buffer: Mutex::new(BufferState {
                findings: Vec::new(),
                first_buffered_at: None,
            }),
            batch_size: config.batch_size,
            batch_window: Duration::from_millis(config.batch_window_ms),
            max_retries: 0,
        });
        SlackChannel {
            inner,
            _flush_task: tokio::spawn(std::future::pending()),
        }
    }

    fn test_config() -> SlackConfig {
        SlackConfig {
            name: None,
            token: crate::config::Secret::from("xoxb-test-token"),
            channel: "#alerts".to_string(),
            username: Some("pgsense-bot".to_string()),
            icon_emoji: Some(":shield:".to_string()),
            timeout_ms: 3000,
            batch_size: 8,
            batch_window_ms: 2000,
            max_retries: 3,
        }
    }

    #[tokio::test]
    async fn slack_channel_builds_from_config() {
        let channel = SlackChannel::new(&test_config()).unwrap();
        assert_eq!(channel.inner.channel, "#alerts");
        assert_eq!(channel.inner.username.as_deref(), Some("pgsense-bot"));
        assert_eq!(channel.inner.icon_emoji.as_deref(), Some(":shield:"));
        assert_eq!(channel.inner.batch_size, 8);
        assert_eq!(channel.inner.max_retries, 3);
    }

    #[test]
    fn severity_colors_are_distinct() {
        let colors: Vec<&str> = [Severity::Critical, Severity::High, Severity::Medium, Severity::Low, Severity::Info]
            .iter()
            .map(severity_color)
            .collect();

        let unique: std::collections::HashSet<&&str> = colors.iter().collect();
        assert_eq!(unique.len(), 5);
    }

    #[tokio::test]
    async fn single_finding_payload_unchanged() {
        let channel = SlackChannel::new(&test_config()).unwrap();
        let payload = channel.inner.build_payload(&[test_finding()]);
        let attachments = payload["attachments"].as_array().unwrap();
        assert_eq!(attachments.len(), 1);
        let blocks = attachments[0]["blocks"].as_array().unwrap();
        assert_eq!(blocks.len(), 4);
    }

    #[tokio::test]
    async fn batch_payload_groups_findings() {
        let channel = SlackChannel::new(&test_config()).unwrap();
        let findings: Vec<Finding> = (0..3)
            .map(|i| {
                let mut f = test_finding();
                f.column_name = format!("col_{i}");
                f
            })
            .collect();

        let payload = channel.inner.build_payload(&findings);
        let attachments = payload["attachments"].as_array().unwrap();
        assert_eq!(attachments.len(), 1);

        let header_text = attachments[0]["blocks"][0]["text"]["text"]
            .as_str()
            .unwrap();
        assert!(header_text.contains("3 Sensitive Data Findings"));
    }

    #[tokio::test]
    async fn batch_payload_uses_highest_severity_color() {
        let channel = SlackChannel::new(&test_config()).unwrap();
        let mut f1 = test_finding();
        f1.severity = Severity::Low;
        let mut f2 = test_finding();
        f2.severity = Severity::Critical;

        let payload = channel.inner.build_payload(&[f1, f2]);
        let color = payload["attachments"][0]["color"].as_str().unwrap();
        assert_eq!(color, severity_color(&Severity::Critical));
    }

    #[tokio::test]
    async fn send_buffers_finding() {
        let channel = test_channel();

        channel.send(&test_finding()).await.unwrap();

        let buf = channel.inner.buffer.lock().await;
        assert_eq!(buf.findings.len(), 1);
        assert!(buf.first_buffered_at.is_some());
    }

    #[tokio::test]
    async fn send_flushes_at_batch_size() {
        let channel = test_channel();

        for _ in 0..channel.inner.batch_size {
            let _ = channel.send(&test_finding()).await;
        }

        let buf = channel.inner.buffer.lock().await;
        assert!(buf.findings.is_empty());
        assert!(buf.first_buffered_at.is_none());
    }

    #[tokio::test]
    async fn flush_drains_buffer() {
        let channel = test_channel();

        {
            let mut buf = channel.inner.buffer.lock().await;
            buf.findings.push(test_finding());
            buf.first_buffered_at = Some(Instant::now());
        }

        let _ = channel.flush().await; // fails fast — hits closed port

        let buf = channel.inner.buffer.lock().await;
        assert!(buf.findings.is_empty());
    }
}
