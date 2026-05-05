use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};

use crate::{
    alerts::{jsonl::JsonlChannel, postgres::is_valid_identifier},
    config::Secret,
    pipeline::config::TlsSettings,
    validation::Validate,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AlertsConfig {
    pub dedup_window_seconds: u64,
    pub log: LogAlertConfig,
    pub stdout: StdoutAlertConfig,
    pub jsonl: JsonlAlertConfig,
    pub webhooks: Vec<WebhookConfig>,
    pub slack: Vec<SlackConfig>,
    pub postgres: Option<PostgresAlertConfig>,
}

impl Default for AlertsConfig {
    fn default() -> Self {
        Self {
            dedup_window_seconds: 300,
            log: LogAlertConfig::default(),
            stdout: StdoutAlertConfig::default(),
            jsonl: JsonlAlertConfig::default(),
            webhooks: Vec::new(),
            slack: Vec::new(),
            postgres: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PostgresAlertConfig {
    pub name: Option<String>,
    pub host: String,
    pub port: u16,
    pub dbname: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password: Option<Secret>,
    pub schema: String,
    pub table: String,
    pub tls: TlsSettings,
}

impl Default for PostgresAlertConfig {
    fn default() -> Self {
        Self {
            name: None,
            host: "localhost".to_string(),
            port: 5432,
            dbname: "postgres".to_string(),
            username: "postgres".to_string(),
            password: None,
            schema: "pgsense".to_string(),
            table: "findings".to_string(),
            tls: TlsSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct LogAlertConfig {
    pub enabled: bool,
}

impl Default for LogAlertConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct StdoutAlertConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct JsonlAlertConfig {
    pub enabled: bool,
    pub name: Option<String>,
    pub path: PathBuf,
}

impl Default for JsonlAlertConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            name: None,
            path: PathBuf::from("alerts.jsonl"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookConfig {
    #[serde(default)]
    pub name: Option<String>,
    pub url: String,
    #[serde(default, skip_serializing)]
    pub headers: HashMap<String, Secret>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    5000
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SlackConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(skip_serializing)]
    pub token: Secret,
    pub channel: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub icon_emoji: Option<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_batch_window_ms")]
    pub batch_window_ms: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_batch_size() -> usize {
    8
}

fn default_batch_window_ms() -> u64 {
    2000
}

fn default_max_retries() -> u32 {
    3
}

pub enum ChannelRef<'a> {
    Log,
    Stdout,
    Jsonl(&'a JsonlAlertConfig),
    Webhook(&'a WebhookConfig),
    Slack(&'a SlackConfig),
    Postgres(&'a PostgresAlertConfig),
}

impl AlertsConfig {
    /// Iterate over all enabled alert channels paired with their resolved name.
    pub fn channels(&self) -> Vec<(String, ChannelRef<'_>)> {
        let mut out = Vec::new();
        if self.log.enabled {
            out.push(("log".to_string(), ChannelRef::Log));
        }
        if self.stdout.enabled {
            out.push(("stdout".to_string(), ChannelRef::Stdout));
        }
        if self.jsonl.enabled {
            out.push(("jsonl".to_string(), ChannelRef::Jsonl(&self.jsonl)));
        }
        for (i, w) in self.webhooks.iter().enumerate() {
            let name = w
                .name
                .clone()
                .unwrap_or_else(|| format!("webhook-{}", i + 1));
            out.push((name, ChannelRef::Webhook(w)));
        }
        for (i, s) in self.slack.iter().enumerate() {
            let name = s.name.clone().unwrap_or_else(|| format!("slack-{}", i + 1));
            out.push((name, ChannelRef::Slack(s)));
        }
        if let Some(pg) = &self.postgres {
            out.push(("postgres".to_string(), ChannelRef::Postgres(pg)));
        }
        out
    }

    pub fn names(&self) -> HashSet<String> {
        self.channels().into_iter().map(|(n, _)| n).collect()
    }

    /// Collect hard-error messages across all enabled channels. Empty = valid.
    /// Cross-cutting concerns (duplicate names) are intentionally excluded —
    /// the CLI surfaces them as warnings.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for (name, ch) in self.channels() {
            match ch {
                ChannelRef::Log | ChannelRef::Stdout => {},
                ChannelRef::Jsonl(j) => errors.extend(j.validate(&name)),
                ChannelRef::Webhook(w) => errors.extend(w.validate(&name)),
                ChannelRef::Slack(s) => errors.extend(s.validate(&name)),
                ChannelRef::Postgres(pg) => errors.extend(pg.validate(&name)),
            }
        }
        errors
    }
}

impl Validate for WebhookConfig {
    fn validate(&self, name: &str) -> Vec<String> {
        let mut errs = Vec::new();
        if self.url.trim().is_empty() {
            errs.push(format!("webhook '{name}': url is empty"));
        } else if !(self.url.starts_with("http://") || self.url.starts_with("https://")) {
            errs.push(format!("webhook '{name}': url must start with http:// or https://"));
        }
        errs
    }
}

impl Validate for SlackConfig {
    fn validate(&self, name: &str) -> Vec<String> {
        let mut errs = Vec::new();
        // Only check inline secrets here — file-backed tokens are validated by
        // Config::resolve_secrets, which surfaces a clear IO error with the
        // file path. Don't double-validate.
        if let Secret::Inline(s) = &self.token
            && s.expose_secret().is_empty()
        {
            errs.push(format!("slack '{name}': token is empty"));
        }
        if self.channel.trim().is_empty() {
            errs.push(format!("slack '{name}': channel is empty"));
        }
        errs
    }
}

impl Validate for PostgresAlertConfig {
    fn validate(&self, name: &str) -> Vec<String> {
        let mut errs = Vec::new();
        if !is_valid_identifier(&self.schema) {
            errs.push(format!(
                "postgres '{name}': invalid schema name '{}' (must be ASCII alphanumeric or underscore)",
                self.schema
            ));
        }
        if !is_valid_identifier(&self.table) {
            errs.push(format!(
                "postgres '{name}': invalid table name '{}' (must be ASCII alphanumeric or underscore)",
                self.table
            ));
        }
        errs
    }
}

impl Validate for JsonlAlertConfig {
    fn validate(&self, name: &str) -> Vec<String> {
        match JsonlChannel::new(self) {
            Ok(_) => Vec::new(),
            Err(e) => vec![format!("jsonl '{name}': {e:#}")],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod channel_names {
        use super::*;

        #[test]
        fn default_log_only() {
            let alerts = AlertsConfig::default();
            let names = alerts.names();
            assert_eq!(names, ["log".to_string()].into_iter().collect());
        }

        #[test]
        fn full_set() {
            let alerts = AlertsConfig {
                log: LogAlertConfig { enabled: true },
                stdout: StdoutAlertConfig { enabled: true },
                jsonl: JsonlAlertConfig {
                    enabled: true,
                    ..Default::default()
                },
                webhooks: vec![WebhookConfig {
                    name: None,
                    url: "https://example.com".into(),
                    headers: Default::default(),
                    timeout_ms: 1000,
                }],
                slack: vec![],
                postgres: Some(PostgresAlertConfig::default()),
                ..Default::default()
            };
            let names = alerts.names();
            assert!(names.contains("log"));
            assert!(names.contains("stdout"));
            assert!(names.contains("jsonl"));
            assert!(names.contains("webhook-1"));
            assert!(names.contains("postgres"));
        }

        #[test]
        fn indexed_when_multiple() {
            let alerts = AlertsConfig {
                webhooks: vec![
                    WebhookConfig {
                        name: None,
                        url: "https://a".into(),
                        headers: Default::default(),
                        timeout_ms: 1000,
                    },
                    WebhookConfig {
                        name: None,
                        url: "https://b".into(),
                        headers: Default::default(),
                        timeout_ms: 1000,
                    },
                ],
                ..Default::default()
            };
            let names = alerts.names();
            assert!(names.contains("webhook-1"));
            assert!(names.contains("webhook-2"));
        }
    }

    mod webhook_validate {
        use super::*;

        #[test]
        fn missing_scheme() {
            let w = WebhookConfig {
                name: Some("hook".into()),
                url: "example.com".into(),
                headers: Default::default(),
                timeout_ms: 1000,
            };
            let errs = w.validate("hook");
            assert_eq!(errs.len(), 1);
            assert!(errs[0].contains("http://") || errs[0].contains("https://"));
        }

        #[test]
        fn empty_url() {
            let w = WebhookConfig {
                name: None,
                url: "  ".into(),
                headers: Default::default(),
                timeout_ms: 1000,
            };
            let errs = w.validate("hook");
            assert_eq!(errs.len(), 1);
            assert!(errs[0].contains("url is empty"));
        }
    }

    mod slack_validate {
        use super::*;

        #[test]
        fn empty_token() {
            let s = SlackConfig {
                name: None,
                token: Secret::from(""),
                channel: "#x".into(),
                username: None,
                icon_emoji: None,
                timeout_ms: 1000,
                batch_size: 1,
                batch_window_ms: 1000,
                max_retries: 0,
            };
            let errs = s.validate("s");
            assert!(errs.iter().any(|e| e.contains("token is empty")));
        }
    }

    mod postgres_validate {
        use super::*;

        #[test]
        fn invalid_schema() {
            let pg = PostgresAlertConfig {
                schema: "bad-schema".into(),
                ..Default::default()
            };
            let errs = pg.validate("postgres");
            assert!(errs.iter().any(|e| e.contains("invalid schema name")));
        }
    }

    mod alerts_validate {
        use super::*;

        #[test]
        fn aggregates_per_channel_errors() {
            let alerts = AlertsConfig {
                postgres: Some(PostgresAlertConfig {
                    schema: "bad-schema".into(),
                    ..Default::default()
                }),
                webhooks: vec![WebhookConfig {
                    name: Some("hook".into()),
                    url: "ftp://nope".into(),
                    headers: Default::default(),
                    timeout_ms: 1000,
                }],
                ..Default::default()
            };
            let errs = alerts.validate();
            assert_eq!(errs.len(), 2);
        }

        #[test]
        fn default_is_clean() {
            let alerts = AlertsConfig::default();
            assert!(alerts.validate().is_empty());
        }
    }
}
