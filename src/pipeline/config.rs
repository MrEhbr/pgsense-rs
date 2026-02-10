use anyhow::{Result, bail};
use etl::config::{BatchConfig, PgConnectionConfig, PipelineConfig, TableSyncCopyConfig, TlsConfig};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub dbname: String,
    pub username: String,
    pub password: Option<String>,
    pub publication: String,
    pub tls: TlsSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct TlsSettings {
    pub enabled: bool,
    pub trusted_root_certs: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SqliteStoreConfig {
    pub path: String,
}

impl Default for SqliteStoreConfig {
    fn default() -> Self {
        Self {
            path: "pgsense-state.db".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PostgresStoreConfig {
    pub host: String,
    pub port: u16,
    pub dbname: String,
    pub username: String,
    pub password: Option<String>,
    pub schema: String,
    pub tls: TlsSettings,
}

impl Default for PostgresStoreConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            dbname: "postgres".to_string(),
            username: "postgres".to_string(),
            password: None,
            schema: "pgsense".to_string(),
            tls: TlsSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StoreType {
    #[default]
    Memory,
    Postgres(PostgresStoreConfig),
    Sqlite(SqliteStoreConfig),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PipelineSettings {
    pub store: StoreType,
    pub batch_max_size: usize,
    pub batch_max_fill_ms: u64,
    pub table_error_retry_delay_ms: u64,
    pub table_error_retry_max_attempts: u32,
    pub max_table_sync_workers: u16,
}

impl PostgresConfig {
    pub fn to_pg_connection_config(&self) -> PgConnectionConfig {
        PgConnectionConfig {
            host: self.host.clone(),
            port: self.port,
            name: self.dbname.clone(),
            username: self.username.clone(),
            password: self.password.clone().map(SecretString::from),
            tls: TlsConfig {
                enabled: self.tls.enabled,
                trusted_root_certs: self.tls.trusted_root_certs.clone(),
            },
            keepalive: None,
        }
    }
}

impl PipelineSettings {
    pub fn to_pipeline_config(&self, id: u64, publication_name: &str, pg_connection: PgConnectionConfig) -> Result<PipelineConfig> {
        if publication_name.is_empty() {
            bail!("publication name must not be empty");
        }

        Ok(PipelineConfig {
            id,
            publication_name: publication_name.to_string(),
            pg_connection,
            batch: BatchConfig {
                max_size: self.batch_max_size,
                max_fill_ms: self.batch_max_fill_ms,
            },
            table_error_retry_delay_ms: self.table_error_retry_delay_ms,
            table_error_retry_max_attempts: self.table_error_retry_max_attempts,
            max_table_sync_workers: self.max_table_sync_workers,
            table_sync_copy: TableSyncCopyConfig::SkipAllTables,
        })
    }
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            dbname: "postgres".to_string(),
            username: "postgres".to_string(),
            password: None,
            publication: "pgsense_pub".to_string(),
            tls: TlsSettings::default(),
        }
    }
}

impl Default for PipelineSettings {
    fn default() -> Self {
        Self {
            store: StoreType::default(),
            batch_max_size: 1000,
            batch_max_fill_ms: 5000,
            table_error_retry_delay_ms: 10000,
            table_error_retry_max_attempts: 5,
            max_table_sync_workers: 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_publication_rejected() {
        let settings = PipelineSettings::default();
        let pg_config = PostgresConfig::default().to_pg_connection_config();

        let result = settings.to_pipeline_config(1, "", pg_config);
        assert!(result.is_err());
    }
}
