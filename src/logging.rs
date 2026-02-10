use std::{fs::OpenOptions, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tracing::{Level, level_filters::LevelFilter};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

fn serialize_level<S>(level: &Option<Level>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match level {
        Some(l) => serializer.serialize_str(&l.to_string().to_lowercase()),
        None => serializer.serialize_none(),
    }
}

fn deserialize_level<'de, D>(deserializer: D) -> Result<Option<Level>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = Option::<String>::deserialize(deserializer)?;
    match s {
        Some(s) => Ok(Some(s.parse::<Level>().map_err(serde::de::Error::custom)?)),
        None => Ok(None),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Console,
    Json,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogOutput {
    #[default]
    Stderr,
    Stdout,
    File(PathBuf),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogConfig {
    #[serde(serialize_with = "serialize_level", deserialize_with = "deserialize_level")]
    pub level: Option<Level>,
    #[serde(default)]
    pub format: LogFormat,
    #[serde(default)]
    pub output: LogOutput,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: Some(Level::INFO),
            format: LogFormat::Console,
            output: LogOutput::Stderr,
        }
    }
}

/// Returns a WorkerGuard that must be held until program exit to ensure log
/// flush.
pub fn setup(config: &LogConfig) -> Result<WorkerGuard> {
    let level_filter: LevelFilter = config.level.into();

    let env_filter = EnvFilter::builder()
        .with_default_directive(level_filter.into())
        .with_env_var("RUST_LOG")
        .from_env_lossy()
        .add_directive("hyper=warn".parse().unwrap())
        .add_directive("h2=warn".parse().unwrap())
        .add_directive("tower=warn".parse().unwrap())
        .add_directive("reqwest::connect=warn".parse().unwrap())
        .add_directive("ureq=warn".parse().unwrap())
        .add_directive("rustls=warn".parse().unwrap())
        .add_directive("want=warn".parse().unwrap())
        .add_directive("mio=warn".parse().unwrap())
        .add_directive("tokio=warn".parse().unwrap())
        .add_directive("etl=warn".parse().unwrap())
        .add_directive("etl_replicator=warn".parse().unwrap())
        .add_directive("etl_postgres=warn".parse().unwrap())
        .add_directive("etl_config=warn".parse().unwrap())
        .add_directive("etl_destinations=warn".parse().unwrap())
        .add_directive("etl_telemetry=warn".parse().unwrap())
        .add_directive("etl_api=warn".parse().unwrap());

    let (writer, guard) = match &config.output {
        LogOutput::Stderr => tracing_appender::non_blocking(std::io::stderr()),
        LogOutput::Stdout => tracing_appender::non_blocking(std::io::stdout()),
        LogOutput::File(path) => {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .with_context(|| format!("Failed to open log file: {}", path.display()))?;
            tracing_appender::non_blocking(file)
        },
    };

    let ansi = !matches!(config.output, LogOutput::File(_));

    let fmt_layer = match config.format {
        LogFormat::Console => tracing_subscriber::fmt::layer()
            .compact()
            .with_line_number(true)
            .with_writer(writer)
            .with_ansi(ansi)
            .boxed(),
        LogFormat::Json => tracing_subscriber::fmt::layer()
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(false)
            .with_writer(writer)
            .boxed(),
    }
    .with_filter(env_filter);

    let registry = tracing_subscriber::registry().with(fmt_layer);

    #[cfg(feature = "tokio-console")]
    let registry = registry.with(console_subscriber::spawn());

    registry
        .try_init()
        .context("Failed to initialize logging")?;

    Ok(guard)
}
