use std::path::PathBuf;

use anyhow::{Result, bail};
use etl::config::{BatchConfig, PgConnectionConfig, PipelineConfig, TableSyncCopyConfig, TlsConfig};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

use crate::scanner::ScanFilter;

/// Per-database connection configuration. Each `[[databases]]` entry in the
/// config file maps to one of these.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub dbname: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password: Option<SecretString>,
    /// Path to a file containing the password. Takes precedence over
    /// `password` if set.
    #[serde(skip_serializing)]
    pub password_file: Option<PathBuf>,
    pub publication: String,
    pub tls: TlsSettings,
    /// Optional per-database scan filter. Overrides the top-level `[scan]`
    /// config when set.
    pub scan: Option<ScanFilter>,
}

impl DatabaseConfig {
    /// Stable identifier for this database connection: `"{host}/{dbname}"`.
    pub fn database_id(&self) -> String {
        format!("{}/{}", self.host, self.dbname)
    }

    /// FNV-1a 64-bit hash of `database_id()` — stable across Rust versions,
    /// used for replication slot IDs.
    pub fn pipeline_id(&self) -> u64 {
        const FNV_OFFSET: u64 = 14695981039346656037;
        const FNV_PRIME: u64 = 1099511628211;
        let mut hash = FNV_OFFSET;
        for byte in self.database_id().bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }

    pub fn to_pg_connection_config(&self) -> PgConnectionConfig {
        PgConnectionConfig {
            host: self.host.clone(),
            port: self.port,
            name: self.dbname.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
            tls: TlsConfig {
                enabled: self.tls.enabled,
                trusted_root_certs: self.tls.trusted_root_certs.clone(),
            },
            keepalive: None,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            dbname: "postgres".to_string(),
            username: "postgres".to_string(),
            password: None,
            password_file: None,
            publication: "pgsense_pub".to_string(),
            tls: TlsSettings::default(),
            scan: None,
        }
    }
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
    #[serde(skip_serializing)]
    pub password: Option<SecretString>,
    #[serde(skip_serializing)]
    pub password_file: Option<PathBuf>,
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
            password_file: None,
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
        let pg_connection = DatabaseConfig::default().to_pg_connection_config();

        let result = settings.to_pipeline_config(1, "", pg_connection);
        assert!(result.is_err());
    }

    #[test]
    fn test_database_id() {
        let db = DatabaseConfig {
            host: "db1.example.com".to_string(),
            dbname: "orders".to_string(),
            ..Default::default()
        };
        assert_eq!(db.database_id(), "db1.example.com/orders");
    }

    #[test]
    fn pipeline_id_is_deterministic() {
        let db1 = DatabaseConfig {
            host: "localhost".into(),
            dbname: "postgres".into(),
            ..Default::default()
        };
        let db2 = DatabaseConfig {
            host: "localhost".into(),
            dbname: "other".into(),
            ..Default::default()
        };
        assert_eq!(db1.pipeline_id(), db1.pipeline_id());
        assert_ne!(db1.pipeline_id(), db2.pipeline_id());
    }
}
