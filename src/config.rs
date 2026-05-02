use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use config::{Environment, File, FileFormat};
use secrecy::SecretString;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    alerts::config::AlertsConfig,
    logging::LogConfig,
    pipeline::config::{DatabaseConfig, PipelineSettings, StoreType},
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
    /// Load config from file + env vars, resolve `password_file` fields,
    /// apply global scan filter defaults, and validate.
    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        let mut config: Self = load(config_path)?;
        config
            .resolve_passwords()
            .context("failed to resolve password files")?;
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

        match &self.pipeline.store {
            StoreType::Memory => {},
            StoreType::Postgres(cfg) => errs.extend(cfg.validate("postgres-store")),
            StoreType::Sqlite(cfg) => errs.extend(cfg.validate("sqlite-store")),
        }

        errs.extend(self.alerts.validate());

        if !errs.is_empty() {
            bail!("config invalid:\n  - {}", errs.join("\n  - "));
        }
        Ok(())
    }

    /// Resolve all `password_file` fields across databases, store, and alert
    /// configs. Call after loading config and before using connections.
    pub fn resolve_passwords(&mut self) -> Result<()> {
        for db in &mut self.databases {
            if let Some(path) = &db.password_file {
                db.password = Some(read_password_file(path)?);
            }
        }

        if let StoreType::Postgres(ref mut pg) = self.pipeline.store
            && let Some(path) = &pg.password_file
        {
            pg.password = Some(read_password_file(path)?);
        }

        if let Some(ref mut pg) = self.alerts.postgres
            && let Some(path) = &pg.password_file
        {
            pg.password = Some(read_password_file(path)?);
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

/// Read password from a file, trimming trailing whitespace/newlines.
fn read_password_file(path: &Path) -> Result<SecretString> {
    let content = std::fs::read_to_string(path).with_context(|| format!("failed to read password file: {}", path.display()))?;
    Ok(SecretString::from(content.trim_end().to_string()))
}

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
    use std::io::Write;

    use secrecy::ExposeSecret;
    use tempfile::NamedTempFile;

    use super::*;
    use crate::pipeline::config::DatabaseConfig;

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
    fn resolve_passwords_reads_from_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "s3cret").unwrap();

        let mut config = Config {
            databases: vec![DatabaseConfig {
                password_file: Some(file.path().to_path_buf()),
                ..Default::default()
            }],
            ..Default::default()
        };
        config.resolve_passwords().unwrap();
        assert_eq!(
            config.databases[0]
                .password
                .as_ref()
                .unwrap()
                .expose_secret(),
            "s3cret"
        );
    }

    #[test]
    fn resolve_passwords_trims_trailing_newline() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "pass\n\n").unwrap();

        let mut config = Config {
            databases: vec![DatabaseConfig {
                password_file: Some(file.path().to_path_buf()),
                ..Default::default()
            }],
            ..Default::default()
        };
        config.resolve_passwords().unwrap();
        assert_eq!(
            config.databases[0]
                .password
                .as_ref()
                .unwrap()
                .expose_secret(),
            "pass"
        );
    }

    #[test]
    fn resolve_passwords_missing_file_errors() {
        let mut config = Config {
            databases: vec![DatabaseConfig {
                password_file: Some(PathBuf::from("/nonexistent/password")),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(config.resolve_passwords().is_err());
    }

    #[test]
    fn resolve_passwords_noop_without_password_file() {
        let mut config = Config {
            databases: vec![DatabaseConfig {
                password: Some(SecretString::from("inline")),
                ..Default::default()
            }],
            ..Default::default()
        };
        config.resolve_passwords().unwrap();
        assert_eq!(
            config.databases[0]
                .password
                .as_ref()
                .unwrap()
                .expose_secret(),
            "inline"
        );
    }
}
