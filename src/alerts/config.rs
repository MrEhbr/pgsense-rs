use std::{collections::HashMap, path::PathBuf};

use secrecy::SecretString;
use serde::{Deserialize, Serialize};

use crate::pipeline::config::TlsSettings;

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
    pub password: Option<SecretString>,
    #[serde(skip_serializing)]
    pub password_file: Option<PathBuf>,
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
            password_file: None,
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
    pub headers: HashMap<String, SecretString>,
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
