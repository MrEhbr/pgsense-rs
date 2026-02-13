mod queries;
mod types;

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use etl::{
    error::{ErrorKind, EtlResult},
    etl_error,
    state::table::TableReplicationPhase,
    store::{cleanup::CleanupStore, schema::SchemaStore, state::StateStore},
};
use etl_postgres::types::{TableId, TableSchema};
use secrecy::ExposeSecret;
use sqlx::{
    Executor, PgPool,
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
};
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::pipeline::config::PostgresStoreConfig;

const MAX_POOL_CONNECTIONS: u32 = 2;
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
struct Inner {
    table_replication_states: BTreeMap<TableId, TableReplicationPhase>,
    table_state_history: HashMap<TableId, Vec<TableReplicationPhase>>,
    table_schemas: HashMap<TableId, Arc<TableSchema>>,
    table_mappings: HashMap<TableId, String>,
}

/// Dual-layer pattern (mirrors etl's PostgresStore):
/// - In-memory cache for fast `get_*` reads
/// - Postgres DB-first writes on every `update_*`/`store_*` call
/// - `load_*` methods populate cache from Postgres at startup
#[derive(Clone)]
pub struct PostgresStore {
    pipeline_id: u64,
    pool: PgPool,
    inner: Arc<Mutex<Inner>>,
}

impl PostgresStore {
    /// Sets up the connection pool with `search_path` pointing to the
    /// configured schema, creates the schema if it doesn't exist, and runs
    /// migrations.
    pub async fn new(pipeline_id: u64, config: &PostgresStoreConfig) -> EtlResult<Self> {
        let ssl_mode = if config.tls.enabled { PgSslMode::VerifyFull } else { PgSslMode::Prefer };
        let mut connect_options = PgConnectOptions::new()
            .host(&config.host)
            .port(config.port)
            .database(&config.dbname)
            .username(&config.username)
            .ssl_mode(ssl_mode);

        if let Some(password) = &config.password {
            connect_options = connect_options.password(password.expose_secret());
        }

        let schema = &config.schema;
        if !schema
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(etl_error!(
                ErrorKind::ConfigError,
                "Invalid schema name: must contain only ASCII alphanumeric characters and underscores"
            ));
        }
        let schema = schema.clone();

        let pool = PgPoolOptions::new()
            .min_connections(0)
            .max_connections(MAX_POOL_CONNECTIONS)
            .idle_timeout(Some(IDLE_TIMEOUT))
            .after_connect({
                let schema = schema.clone();
                move |conn, _meta| {
                    let schema = schema.clone();
                    Box::pin(async move {
                        conn.execute(format!(r#"SET search_path TO "{schema}""#).as_str())
                            .await?;
                        Ok(())
                    })
                }
            })
            .connect_with(connect_options)
            .await?;

        pool.execute(format!(r#"CREATE SCHEMA IF NOT EXISTS "{schema}""#).as_str())
            .await
            .map_err(|e| etl_error!(ErrorKind::ConfigError, "Failed to create schema", e.to_string()))?;

        sqlx::migrate!("src/pipeline/store/postgres/migrations")
            .run(&pool)
            .await
            .map_err(|e| etl_error!(ErrorKind::ConfigError, "Postgres migration failed", e.to_string()))?;

        info!(schema = %config.schema, "Postgres store initialized");

        Ok(Self {
            pipeline_id,
            pool,
            inner: Arc::new(Mutex::new(Inner {
                table_replication_states: BTreeMap::new(),
                table_state_history: HashMap::new(),
                table_schemas: HashMap::new(),
                table_mappings: HashMap::new(),
            })),
        })
    }
}

// ---------------------------------------------------------------------------
// StateStore
// ---------------------------------------------------------------------------

impl StateStore for PostgresStore {
    async fn get_table_replication_state(&self, table_id: TableId) -> EtlResult<Option<TableReplicationPhase>> {
        let inner = self.inner.lock().await;
        Ok(inner.table_replication_states.get(&table_id).cloned())
    }

    async fn get_table_replication_states(&self) -> EtlResult<BTreeMap<TableId, TableReplicationPhase>> {
        let inner = self.inner.lock().await;
        Ok(inner.table_replication_states.clone())
    }

    async fn load_table_replication_states(&self) -> EtlResult<usize> {
        debug!("loading table replication states from postgres store");

        let rows = queries::get_table_replication_state_rows(&self.pool, self.pipeline_id as i64).await?;

        let mut table_states: BTreeMap<TableId, TableReplicationPhase> = BTreeMap::new();
        for row in rows {
            let table_id = TableId::new(row.table_id.0);
            let phase: TableReplicationPhase = row.try_into()?;
            table_states.insert(table_id, phase);
        }

        let count = table_states.len();
        let mut inner = self.inner.lock().await;
        inner.table_replication_states = table_states;

        info!(count, "loaded table replication states from postgres store");
        Ok(count)
    }

    async fn update_table_replication_state(&self, table_id: TableId, state: TableReplicationPhase) -> EtlResult<()> {
        queries::update_replication_state(&self.pool, self.pipeline_id as i64, table_id, &state).await?;

        let mut inner = self.inner.lock().await;
        if let Some(current) = inner.table_replication_states.get(&table_id).cloned() {
            inner
                .table_state_history
                .entry(table_id)
                .or_default()
                .push(current);
        }
        inner.table_replication_states.insert(table_id, state);

        Ok(())
    }

    async fn rollback_table_replication_state(&self, table_id: TableId) -> EtlResult<TableReplicationPhase> {
        let restored_row = queries::rollback_replication_state(&self.pool, self.pipeline_id as i64, table_id)
            .await?
            .ok_or_else(|| etl_error!(ErrorKind::StateRollbackError, "No previous state available to roll back to"))?;

        let restored_phase: TableReplicationPhase = restored_row.try_into()?;

        let mut inner = self.inner.lock().await;
        inner
            .table_replication_states
            .insert(table_id, restored_phase.clone());
        if let Some(history) = inner.table_state_history.get_mut(&table_id) {
            history.pop();
        }

        Ok(restored_phase)
    }

    async fn get_table_mapping(&self, source_table_id: &TableId) -> EtlResult<Option<String>> {
        let inner = self.inner.lock().await;
        Ok(inner.table_mappings.get(source_table_id).cloned())
    }

    async fn get_table_mappings(&self) -> EtlResult<HashMap<TableId, String>> {
        let inner = self.inner.lock().await;
        Ok(inner.table_mappings.clone())
    }

    async fn load_table_mappings(&self) -> EtlResult<usize> {
        debug!("loading table mappings from postgres store");

        let mappings = queries::load_table_mappings(&self.pool, self.pipeline_id as i64).await?;

        let count = mappings.len();
        let mut inner = self.inner.lock().await;
        inner.table_mappings = mappings;

        info!(count, "loaded table mappings from postgres store");
        Ok(count)
    }

    async fn store_table_mapping(&self, source_table_id: TableId, destination_table_id: String) -> EtlResult<()> {
        queries::store_table_mapping(&self.pool, self.pipeline_id as i64, source_table_id, &destination_table_id).await?;

        let mut inner = self.inner.lock().await;
        inner
            .table_mappings
            .insert(source_table_id, destination_table_id);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SchemaStore
// ---------------------------------------------------------------------------

impl SchemaStore for PostgresStore {
    async fn get_table_schema(&self, table_id: &TableId) -> EtlResult<Option<Arc<TableSchema>>> {
        let inner = self.inner.lock().await;
        Ok(inner.table_schemas.get(table_id).cloned())
    }

    async fn get_table_schemas(&self) -> EtlResult<Vec<Arc<TableSchema>>> {
        let inner = self.inner.lock().await;
        Ok(inner.table_schemas.values().cloned().collect())
    }

    async fn load_table_schemas(&self) -> EtlResult<usize> {
        debug!("loading table schemas from postgres store");

        let schemas = queries::load_table_schemas(&self.pool, self.pipeline_id as i64).await?;

        let count = schemas.len();
        let mut inner = self.inner.lock().await;
        inner.table_schemas.clear();
        for schema in schemas {
            inner.table_schemas.insert(schema.id, Arc::new(schema));
        }

        info!(count, "loaded table schemas from postgres store");
        Ok(count)
    }

    async fn store_table_schema(&self, table_schema: TableSchema) -> EtlResult<()> {
        queries::store_table_schema(&self.pool, self.pipeline_id as i64, &table_schema).await?;

        let mut inner = self.inner.lock().await;
        inner
            .table_schemas
            .insert(table_schema.id, Arc::new(table_schema));

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CleanupStore
// ---------------------------------------------------------------------------

impl CleanupStore for PostgresStore {
    async fn cleanup_table_state(&self, table_id: TableId) -> EtlResult<()> {
        let pid = self.pipeline_id as i64;

        let mut tx = self.pool.begin().await?;

        queries::delete_table_mappings_for_table(&mut *tx, pid, table_id).await?;
        queries::delete_table_schema_for_table(&mut *tx, pid, table_id).await?;
        queries::delete_replication_state_for_table(&mut *tx, pid, table_id).await?;

        tx.commit().await?;

        let mut inner = self.inner.lock().await;
        inner.table_replication_states.remove(&table_id);
        inner.table_state_history.remove(&table_id);
        inner.table_schemas.remove(&table_id);
        inner.table_mappings.remove(&table_id);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use etl_postgres::{
        replication::schema::string_to_postgres_type,
        types::{ColumnSchema, TableName},
    };
    use secrecy::SecretString;
    use testcontainers_modules::{
        postgres::Postgres,
        testcontainers::{ImageExt, runners::AsyncRunner},
    };

    use super::*;

    async fn test_store() -> (PostgresStore, impl std::any::Any) {
        let container = Postgres::default()
            .with_tag("16-alpine")
            .start()
            .await
            .expect("failed to start postgres container");

        let host = container
            .get_host()
            .await
            .expect("failed to get host")
            .to_string();
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("failed to get port");

        let config = PostgresStoreConfig {
            host: host.clone(),
            port,
            dbname: "postgres".to_string(),
            username: "postgres".to_string(),
            password: Some(SecretString::from("postgres")),
            schema: "pgsense_test".to_string(),
            tls: Default::default(),
        };

        let mut last_err = None;
        for _ in 0..10 {
            match PostgresStore::new(1, &config).await {
                Ok(store) => return (store, container),
                Err(e) => {
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                },
            }
        }
        panic!("failed to create postgres store after retries: {:?}", last_err);
    }

    #[cfg_attr(not(docker), ignore = "Docker daemon not available")]
    #[tokio::test]
    async fn test_update_and_get_replication_state() {
        let (store, _container) = test_store().await;
        let table_id = TableId::new(42);

        store
            .update_table_replication_state(table_id, TableReplicationPhase::Init)
            .await
            .unwrap();

        let state = store.get_table_replication_state(table_id).await.unwrap();
        assert_eq!(state, Some(TableReplicationPhase::Init));
    }

    #[cfg_attr(not(docker), ignore = "Docker daemon not available")]
    #[tokio::test]
    async fn test_rollback_state() {
        let (store, _container) = test_store().await;
        let table_id = TableId::new(20);

        store
            .update_table_replication_state(table_id, TableReplicationPhase::Init)
            .await
            .unwrap();
        store
            .update_table_replication_state(table_id, TableReplicationPhase::DataSync)
            .await
            .unwrap();

        let restored = store
            .rollback_table_replication_state(table_id)
            .await
            .unwrap();
        assert_eq!(restored, TableReplicationPhase::Init);

        let current = store.get_table_replication_state(table_id).await.unwrap();
        assert_eq!(current, Some(TableReplicationPhase::Init));
    }

    #[cfg_attr(not(docker), ignore = "Docker daemon not available")]
    #[tokio::test]
    async fn test_rollback_no_previous_state_errors() {
        let (store, _container) = test_store().await;
        let table_id = TableId::new(30);

        store
            .update_table_replication_state(table_id, TableReplicationPhase::Init)
            .await
            .unwrap();

        let result = store.rollback_table_replication_state(table_id).await;
        assert!(result.is_err());
    }

    #[cfg_attr(not(docker), ignore = "Docker daemon not available")]
    #[tokio::test]
    async fn test_load_persists_states() {
        let (store, _container) = test_store().await;
        let t1 = TableId::new(1);
        let t2 = TableId::new(2);

        store
            .update_table_replication_state(t1, TableReplicationPhase::Init)
            .await
            .unwrap();
        store
            .update_table_replication_state(t2, TableReplicationPhase::Ready)
            .await
            .unwrap();

        {
            let mut inner = store.inner.lock().await;
            inner.table_replication_states.clear();
        }

        let count = store.load_table_replication_states().await.unwrap();
        assert_eq!(count, 2);

        let state1 = store.get_table_replication_state(t1).await.unwrap();
        assert_eq!(state1, Some(TableReplicationPhase::Init));

        let state2 = store.get_table_replication_state(t2).await.unwrap();
        assert_eq!(state2, Some(TableReplicationPhase::Ready));
    }

    #[cfg_attr(not(docker), ignore = "Docker daemon not available")]
    #[tokio::test]
    async fn test_load_table_mappings_from_db() {
        let (store, _container) = test_store().await;
        let t1 = TableId::new(1);
        let t2 = TableId::new(2);

        store
            .store_table_mapping(t1, "dest_a".to_string())
            .await
            .unwrap();
        store
            .store_table_mapping(t2, "dest_b".to_string())
            .await
            .unwrap();

        {
            let mut inner = store.inner.lock().await;
            inner.table_mappings.clear();
        }

        let count = store.load_table_mappings().await.unwrap();
        assert_eq!(count, 2);

        assert_eq!(store.get_table_mapping(&t1).await.unwrap(), Some("dest_a".to_string()));
    }

    #[cfg_attr(not(docker), ignore = "Docker daemon not available")]
    #[tokio::test]
    async fn test_cleanup_table_state() {
        let (store, _container) = test_store().await;
        let table_id = TableId::new(99);

        store
            .update_table_replication_state(table_id, TableReplicationPhase::Init)
            .await
            .unwrap();
        store
            .store_table_mapping(table_id, "cleanup_test".to_string())
            .await
            .unwrap();

        let schema = TableSchema::new(
            table_id,
            TableName::new("public".to_string(), "cleanup_test".to_string()),
            vec![ColumnSchema::new("id".to_string(), string_to_postgres_type("INT4"), -1, false, true)],
        );
        store.store_table_schema(schema).await.unwrap();

        store.cleanup_table_state(table_id).await.unwrap();

        assert!(
            store
                .get_table_replication_state(table_id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(store.get_table_mapping(&table_id).await.unwrap().is_none());
        assert!(store.get_table_schema(&table_id).await.unwrap().is_none());
    }

    #[cfg_attr(not(docker), ignore = "Docker daemon not available")]
    #[tokio::test]
    async fn test_load_table_schemas_from_db() {
        let (store, _container) = test_store().await;
        let table_id = TableId::new(200);

        let schema = TableSchema::new(
            table_id,
            TableName::new("public".to_string(), "orders".to_string()),
            vec![
                ColumnSchema::new("id".to_string(), string_to_postgres_type("INT8"), -1, false, true),
                ColumnSchema::new("amount".to_string(), string_to_postgres_type("NUMERIC"), -1, true, false),
            ],
        );
        store.store_table_schema(schema).await.unwrap();

        {
            let mut inner = store.inner.lock().await;
            inner.table_schemas.clear();
        }

        let count = store.load_table_schemas().await.unwrap();
        assert_eq!(count, 1);

        let loaded = store.get_table_schema(&table_id).await.unwrap().unwrap();
        assert_eq!(loaded.column_schemas.len(), 2);
        assert_eq!(loaded.column_schemas[0].name, "id");
        assert_eq!(loaded.column_schemas[0].typ, string_to_postgres_type("INT8"));
    }
}
