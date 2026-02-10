use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use config::{Environment, File, FileFormat};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    alerts::config::AlertsConfig,
    logging::LogConfig,
    pipeline::config::{PipelineSettings, PostgresConfig},
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
    pub postgres: PostgresConfig,
    pub pipeline: PipelineSettings,
    pub rules_file: Option<PathBuf>,
    pub scan: ScanFilter,
    pub alerts: AlertsConfig,
    pub server: ServerConfig,
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
