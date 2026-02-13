use std::{collections::HashMap, fmt, path::PathBuf};

use secrecy::SecretString;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AlertsConfig {
    pub dedup_window_seconds: u64,
    pub log: LogAlertConfig,
    pub stdout: StdoutAlertConfig,
    pub jsonl: JsonlAlertConfig,
    pub webhooks: Vec<WebhookConfig>,
    pub slack: Vec<SlackConfig>,
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
    pub path: PathBuf,
}

impl Default for JsonlAlertConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: PathBuf::from("alerts.jsonl"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookConfig {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    5000
}

#[derive(Clone, Deserialize, Serialize)]
pub struct SlackConfig {
    #[serde(skip_serializing)]
    pub token: SecretString,
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

impl fmt::Debug for SlackConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SlackConfig")
            .field("token", &"[REDACTED]")
            .field("channel", &self.channel)
            .field("username", &self.username)
            .field("icon_emoji", &self.icon_emoji)
            .field("timeout_ms", &self.timeout_ms)
            .field("batch_size", &self.batch_size)
            .field("batch_window_ms", &self.batch_window_ms)
            .field("max_retries", &self.max_retries)
            .finish()
    }
}
