use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use config::{Environment, File, FileFormat};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub mod secret;

pub use secret::Secret;

use crate::{
    alerts::config::AlertsConfig,
    logging::LogConfig,
    pipeline::config::{DatabaseConfig, PipelineSettings},
    rules::config::RuleConfig,
    scanner::ScanFilter,
    validation::Validate,
};

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OtlpProtocol {
    #[default]
    Grpc,
    Http,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct TelemetryConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub protocol: OtlpProtocol,
    pub service_name: String,
    pub sample_rate: f64,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: "http://localhost:4317".to_string(),
            protocol: OtlpProtocol::Grpc,
            service_name: "pgsense".to_string(),
            sample_rate: 1.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ServerConfig {
    pub enabled: bool,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { enabled: false, port: 9090 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct ProfilingConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct Config {
    pub log: LogConfig,
    pub telemetry: TelemetryConfig,
    pub databases: Vec<DatabaseConfig>,
    pub pipeline: PipelineSettings,
    pub rules_file: Option<PathBuf>,
    pub scan: ScanFilter,
    pub alerts: AlertsConfig,
    pub server: ServerConfig,
    pub profiling: ProfilingConfig,
}

impl Config {
    /// Load config from file + env vars, resolve any file-backed secrets,
    /// apply global scan filter defaults, and validate.
    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        let mut config: Self = load(config_path)?;
        config
            .resolve_secrets()
            .context("failed to resolve secret files")?;
        for db in &mut config.databases {
            if db.scan.is_none() {
                db.scan = Some(config.scan.clone());
            }
        }
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<()> {
        let mut errs = Vec::new();

        if self.databases.is_empty() {
            errs.push("no databases configured — add at least one [[databases]] entry to your config file".to_string());
        }

        let mut seen = HashSet::new();
        for db in &self.databases {
            let id = db.database_id();
            if !seen.insert(id.clone()) {
                errs.push(format!(
                    "duplicate database '{id}' — each host/dbname combination must be unique"
                ));
            }
            errs.extend(db.validate(&id));
        }

        errs.extend(self.alerts.validate());

        if !errs.is_empty() {
            bail!("config invalid:\n  - {}", errs.join("\n  - "));
        }
        Ok(())
    }

    /// Resolve all file-backed secrets across databases and alert configs.
    /// Call after loading config and before using connections.
    pub fn resolve_secrets(&mut self) -> Result<()> {
        for db in &mut self.databases {
            if let Some(secret) = &mut db.password {
                secret.resolve()?;
            }
        }

        if let Some(pg) = &mut self.alerts.postgres
            && let Some(secret) = &mut pg.password
        {
            secret.resolve()?;
        }

        for s in &mut self.alerts.slack {
            s.token.resolve()?;
        }

        for w in &mut self.alerts.webhooks {
            for v in w.headers.values_mut() {
                v.resolve()?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RulesFile {
    #[serde(default)]
    rules: Vec<RuleConfig>,
}

/// The file should contain a top-level `[[rules]]` array.
pub fn load_rules(path: &Path) -> Result<Vec<RuleConfig>> {
    let content = std::fs::read_to_string(path).with_context(|| format!("failed to read rules file: {}", path.display()))?;
    let file: RulesFile = toml::from_str(&content).with_context(|| format!("failed to parse rules file: {}", path.display()))?;
    Ok(file.rules)
}

const ENV_PREFIX: &str = "PGSENSE";

/// Load configuration with precedence: env vars (PGSENSE__*) > config file >
/// defaults.
pub fn load<T>(config_path: Option<&Path>) -> Result<T>
where
    T: DeserializeOwned + Serialize + Default,
{
    let mut builder = config::Config::builder().add_source(config::Config::try_from(&T::default())?);

    if let Some(path) = config_path {
        builder = builder.add_source(File::from(path).format(FileFormat::Toml).required(false));
    }

    let config = builder
        .add_source(Environment::with_prefix(ENV_PREFIX).separator("__"))
        .build()?;

    Ok(config.try_deserialize()?)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, io::Write};

    use secrecy::ExposeSecret;
    use tempfile::NamedTempFile;

    use super::*;
    use crate::{
        alerts::config::{SlackConfig, WebhookConfig},
        pipeline::config::DatabaseConfig,
    };

    fn single_db_config() -> Config {
        Config {
            databases: vec![DatabaseConfig::default()],
            ..Default::default()
        }
    }

    #[test]
    fn validate_empty_databases_rejected() {
        let config = Config::default();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("no databases configured"));
    }

    #[test]
    fn validate_single_database_ok() {
        single_db_config().validate().unwrap();
    }

    #[test]
    fn validate_duplicate_databases_rejected() {
        let config = Config {
            databases: vec![DatabaseConfig::default(), DatabaseConfig::default()],
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("duplicate database"));
    }

    #[test]
    fn validate_distinct_databases_ok() {
        let config = Config {
            databases: vec![
                DatabaseConfig {
                    dbname: "db1".to_string(),
                    ..Default::default()
                },
                DatabaseConfig {
                    dbname: "db2".to_string(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        config.validate().unwrap();
    }

    #[test]
    fn resolve_secrets_reads_db_password_from_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "s3cret").unwrap();

        let mut config = Config {
            databases: vec![DatabaseConfig {
                password: Some(Secret::File {
                    file: file.path().to_path_buf(),
                }),
                ..Default::default()
            }],
            ..Default::default()
        };
        config.resolve_secrets().unwrap();
        assert_eq!(
            config.databases[0]
                .password
                .as_ref()
                .unwrap()
                .expose()
                .expose_secret(),
            "s3cret"
        );
    }

    #[test]
    fn resolve_secrets_trims_trailing_newline() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "pass\n\n").unwrap();

        let mut config = Config {
            databases: vec![DatabaseConfig {
                password: Some(Secret::File {
                    file: file.path().to_path_buf(),
                }),
                ..Default::default()
            }],
            ..Default::default()
        };
        config.resolve_secrets().unwrap();
        assert_eq!(
            config.databases[0]
                .password
                .as_ref()
                .unwrap()
                .expose()
                .expose_secret(),
            "pass"
        );
    }

    #[test]
    fn resolve_secrets_missing_file_errors() {
        let mut config = Config {
            databases: vec![DatabaseConfig {
                password: Some(Secret::File {
                    file: PathBuf::from("/nonexistent/password"),
                }),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(config.resolve_secrets().is_err());
    }

    #[test]
    fn resolve_secrets_noop_for_inline() {
        let mut config = Config {
            databases: vec![DatabaseConfig {
                password: Some(Secret::from("inline")),
                ..Default::default()
            }],
            ..Default::default()
        };
        config.resolve_secrets().unwrap();
        assert_eq!(
            config.databases[0]
                .password
                .as_ref()
                .unwrap()
                .expose()
                .expose_secret(),
            "inline"
        );
    }

    #[test]
    fn resolve_secrets_resolves_slack_token_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "xoxb-from-file").unwrap();

        let mut alerts = AlertsConfig::default();
        alerts.slack.push(SlackConfig {
            name: None,
            token: Secret::File {
                file: file.path().to_path_buf(),
            },
            channel: "#x".into(),
            username: None,
            icon_emoji: None,
            timeout_ms: 1000,
            batch_size: 1,
            batch_window_ms: 1000,
            max_retries: 0,
        });
        let mut config = Config {
            databases: vec![DatabaseConfig::default()],
            alerts,
            ..Default::default()
        };
        config.resolve_secrets().unwrap();
        assert_eq!(config.alerts.slack[0].token.expose().expose_secret(), "xoxb-from-file");
    }

    #[test]
    fn resolve_secrets_resolves_webhook_header_files_and_keeps_inline() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "Bearer abc123").unwrap();

        let mut alerts = AlertsConfig::default();
        alerts.webhooks.push(WebhookConfig {
            name: None,
            url: "https://example.com".into(),
            headers: HashMap::from([
                (
                    "Authorization".to_string(),
                    Secret::File {
                        file: file.path().to_path_buf(),
                    },
                ),
                ("X-Source".to_string(), Secret::from("pgsense")),
            ]),
            timeout_ms: 1000,
        });
        let mut config = Config {
            databases: vec![DatabaseConfig::default()],
            alerts,
            ..Default::default()
        };
        config.resolve_secrets().unwrap();
        let headers = &config.alerts.webhooks[0].headers;
        assert_eq!(headers["Authorization"].expose().expose_secret(), "Bearer abc123");
        assert_eq!(headers["X-Source"].expose().expose_secret(), "pgsense");
    }

    #[test]
    fn resolve_secrets_missing_webhook_header_file_errors() {
        let mut alerts = AlertsConfig::default();
        alerts.webhooks.push(WebhookConfig {
            name: None,
            url: "https://example.com".into(),
            headers: HashMap::from([(
                "Authorization".to_string(),
                Secret::File {
                    file: PathBuf::from("/nonexistent/header"),
                },
            )]),
            timeout_ms: 1000,
        });
        let mut config = Config {
            databases: vec![DatabaseConfig::default()],
            alerts,
            ..Default::default()
        };
        let err = config.resolve_secrets().unwrap_err();
        assert!(err.to_string().contains("failed to read secret file"));
    }
}
