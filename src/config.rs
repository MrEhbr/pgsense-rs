use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use config::{Environment, File, FileFormat};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    alerts::config::AlertsConfig,
    logging::LogConfig,
    pipeline::config::{DatabaseConfig, PipelineSettings},
    rules::config::RuleConfig,
    scanner::ScanFilter,
};

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
pub struct Config {
    pub log: LogConfig,
    pub databases: Vec<DatabaseConfig>,
    pub pipeline: PipelineSettings,
    pub rules_file: Option<PathBuf>,
    pub scan: ScanFilter,
    pub alerts: AlertsConfig,
    pub server: ServerConfig,
}

impl Config {
    /// Returns database configs with the global scan filter applied as default
    /// for databases that don't define their own.
    pub fn databases(&self) -> Vec<DatabaseConfig> {
        self.databases
            .iter()
            .map(|db| {
                let mut db = db.clone();
                if db.scan.is_none() {
                    db.scan = Some(self.scan.clone());
                }
                db
            })
            .collect()
    }

    pub fn validate(&self) -> Result<()> {
        if self.databases.is_empty() {
            bail!("no databases configured — add at least one [[databases]] entry to your config file");
        }

        let mut seen = HashSet::new();
        for db in &self.databases {
            let id = db.database_id();
            if !seen.insert(id.clone()) {
                bail!("duplicate database '{id}' — each host/dbname combination must be unique");
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

const ENV_PREFIX: &str = "APP";

/// Load configuration with precedence: env vars (APP__*) > config file >
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
    fn resolved_databases_applies_global_scan_filter() {
        let config = Config {
            databases: vec![
                DatabaseConfig {
                    dbname: "db1".to_string(),
                    ..Default::default()
                },
                DatabaseConfig {
                    dbname: "db2".to_string(),
                    scan: Some(ScanFilter {
                        include_schemas: vec!["custom".into()],
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            scan: ScanFilter {
                include_schemas: vec!["public".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let resolved = config.databases();
        assert_eq!(resolved[0].scan.as_ref().unwrap().include_schemas, vec!["public".to_string()]);
        assert_eq!(resolved[1].scan.as_ref().unwrap().include_schemas, vec!["custom".to_string()]);
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
}
