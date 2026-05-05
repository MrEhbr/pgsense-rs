use std::fmt::Display;

use anyhow::{Result, bail};
use etl::config::{BatchConfig, InvalidatedSlotBehavior, PgConnectionConfig, PipelineConfig, TableSyncCopyConfig, TcpKeepaliveConfig, TlsConfig};
use serde::{Deserialize, Serialize};

use crate::{config::Secret, scanner::ScanFilter, validation::Validate};

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
    pub password: Option<Secret>,
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
            password: self.password.as_ref().map(|s| s.expose().clone()),
            tls: TlsConfig {
                enabled: self.tls.enabled,
                trusted_root_certs: self.tls.trusted_root_certs.clone(),
            },
            keepalive: TcpKeepaliveConfig::default(),
        }
    }
}

impl Validate for DatabaseConfig {
    fn validate(&self, name: &str) -> Vec<String> {
        let mut errs = Vec::new();
        if self.host.trim().is_empty() {
            errs.push(format!("database '{name}': host is empty"));
        }
        if self.port == 0 {
            errs.push(format!("database '{name}': port must not be 0"));
        }
        if self.dbname.trim().is_empty() {
            errs.push(format!("database '{name}': dbname is empty"));
        }
        if self.username.trim().is_empty() {
            errs.push(format!("database '{name}': username is empty"));
        }
        if self.publication.trim().is_empty() {
            errs.push(format!("database '{name}': publication is empty"));
        }
        errs
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

/// Where the pipeline persists its replication state.
///
/// `Postgres` writes state into the source database under a hardcoded `etl`
/// schema (etl crate's convention) — co-locating state with the source keeps
/// backups and restores consistent.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StoreType {
    #[default]
    Memory,
    Postgres,
}

impl Display for StoreType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreType::Memory => write!(f, "memory"),
            StoreType::Postgres => write!(f, "postgres"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PipelineSettings {
    pub store: StoreType,
    pub batch_max_fill_ms: u64,
    pub batch_memory_budget_ratio: f32,
    pub table_error_retry_delay_ms: u64,
    pub table_error_retry_max_attempts: u32,
    pub max_table_sync_workers: u16,
    pub max_copy_connections_per_table: u16,
    pub memory_refresh_interval_ms: u64,
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
                max_fill_ms: self.batch_max_fill_ms,
                memory_budget_ratio: self.batch_memory_budget_ratio,
            },
            table_error_retry_delay_ms: self.table_error_retry_delay_ms,
            table_error_retry_max_attempts: self.table_error_retry_max_attempts,
            max_table_sync_workers: self.max_table_sync_workers,
            max_copy_connections_per_table: self.max_copy_connections_per_table,
            memory_refresh_interval_ms: self.memory_refresh_interval_ms,
            memory_backpressure: None,
            table_sync_copy: TableSyncCopyConfig::SkipAllTables,
            invalidated_slot_behavior: InvalidatedSlotBehavior::Error,
        })
    }
}

impl Default for PipelineSettings {
    fn default() -> Self {
        Self {
            store: StoreType::default(),
            // Lower than etl's default (10s) — for passive scanning, latency
            // matters and we don't accumulate enough volume to hit the memory
            // budget on its own.
            batch_max_fill_ms: 1000,
            batch_memory_budget_ratio: BatchConfig::DEFAULT_MEMORY_BUDGET_RATIO,
            table_error_retry_delay_ms: PipelineConfig::DEFAULT_TABLE_ERROR_RETRY_DELAY_MS,
            table_error_retry_max_attempts: PipelineConfig::DEFAULT_TABLE_ERROR_RETRY_MAX_ATTEMPTS,
            max_table_sync_workers: PipelineConfig::DEFAULT_MAX_TABLE_SYNC_WORKERS,
            max_copy_connections_per_table: PipelineConfig::DEFAULT_MAX_COPY_CONNECTIONS_PER_TABLE,
            memory_refresh_interval_ms: PipelineConfig::DEFAULT_MEMORY_REFRESH_INTERVAL_MS,
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
