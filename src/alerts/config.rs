use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AlertsConfig {
    pub dedup_window_seconds: u64,
    pub log: LogAlertConfig,
    pub stdout: StdoutAlertConfig,
    pub jsonl: JsonlAlertConfig,
    pub webhooks: Vec<WebhookConfig>,
}

impl Default for AlertsConfig {
    fn default() -> Self {
        Self {
            dedup_window_seconds: 300,
            log: LogAlertConfig::default(),
            stdout: StdoutAlertConfig::default(),
            jsonl: JsonlAlertConfig::default(),
            webhooks: Vec::new(),
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
