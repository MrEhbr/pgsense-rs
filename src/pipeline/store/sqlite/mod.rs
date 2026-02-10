mod queries;
mod types;

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use etl::{
    error::{ErrorKind, EtlResult},
    etl_error,
    state::table::TableReplicationPhase,
    store::{cleanup::CleanupStore, schema::SchemaStore, state::StateStore},
};
use etl_postgres::types::{TableId, TableSchema};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tokio::sync::Mutex;
use tracing::{debug, info};

/// In-memory cache (mirrors MemoryStore's Inner).
#[derive(Debug)]
struct Inner {
    table_replication_states: BTreeMap<TableId, TableReplicationPhase>,
    table_state_history: HashMap<TableId, Vec<TableReplicationPhase>>,
    table_schemas: HashMap<TableId, Arc<TableSchema>>,
    table_mappings: HashMap<TableId, String>,
}

/// Dual-layer pattern (mirrors PostgresStore):
/// - In-memory cache for fast `get_*` reads
/// - SQLite DB-first writes on every `update_*`/`store_*` call
/// - `load_*` methods populate cache from SQLite at startup
#[derive(Clone)]
pub struct SqliteStore {
    pipeline_id: u64,
    pool: SqlitePool,
    inner: Arc<Mutex<Inner>>,
}

impl SqliteStore {
    /// Opens (or creates) a SQLite database at `path` and initializes the
    /// schema.
    pub async fn new(pipeline_id: u64, path: &str) -> EtlResult<Self> {
        let url = if path == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            format!("sqlite:{path}?mode=rwc")
        };

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await?;

        sqlx::query("PRAGMA journal_mode=WAL")
            .execute(&pool)
            .await?;

        sqlx::query("PRAGMA foreign_keys=ON").execute(&pool).await?;

        sqlx::migrate!("src/pipeline/store/sqlite/migrations")
            .run(&pool)
            .await
            .map_err(|e| etl_error!(ErrorKind::ConfigError, "SQLite migration failed", e.to_string()))?;

        info!(path, "SQLite store opened");

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

impl StateStore for SqliteStore {
    async fn get_table_replication_state(&self, table_id: TableId) -> EtlResult<Option<TableReplicationPhase>> {
        let inner = self.inner.lock().await;
        Ok(inner.table_replication_states.get(&table_id).cloned())
    }

    async fn get_table_replication_states(&self) -> EtlResult<BTreeMap<TableId, TableReplicationPhase>> {
        let inner = self.inner.lock().await;
        Ok(inner.table_replication_states.clone())
    }

    async fn load_table_replication_states(&self) -> EtlResult<usize> {
        debug!("loading table replication states from SQLite store");

        let states = queries::load_replication_states(&self.pool, self.pipeline_id as i64).await?;

        let count = states.len();
        let mut inner = self.inner.lock().await;
        inner.table_replication_states = states;

        info!(count, "loaded table replication states from SQLite store");
        Ok(count)
    }

    async fn update_table_replication_state(&self, table_id: TableId, state: TableReplicationPhase) -> EtlResult<()> {
        let pid = self.pipeline_id as i64;
        let tid = table_id.into_inner() as i64;

        queries::upsert_replication_state(&self.pool, pid, tid, &state).await?;

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
        let pid = self.pipeline_id as i64;
        let tid = table_id.into_inner() as i64;

        let restored = queries::rollback_replication_state(&self.pool, pid, tid).await?;

        let mut inner = self.inner.lock().await;
        inner
            .table_replication_states
            .insert(table_id, restored.clone());
        if let Some(history) = inner.table_state_history.get_mut(&table_id) {
            history.pop();
        }

        Ok(restored)
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
        debug!("loading table mappings from SQLite store");

        let mappings = queries::load_mappings(&self.pool, self.pipeline_id as i64).await?;

        let count = mappings.len();
        let mut inner = self.inner.lock().await;
        inner.table_mappings = mappings;

        info!(count, "loaded table mappings from SQLite store");
        Ok(count)
    }

    async fn store_table_mapping(&self, source_table_id: TableId, destination_table_id: String) -> EtlResult<()> {
        let pid = self.pipeline_id as i64;
        let sid = source_table_id.into_inner() as i64;

        queries::upsert_mapping(&self.pool, pid, sid, &destination_table_id).await?;

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

impl SchemaStore for SqliteStore {
    async fn get_table_schema(&self, table_id: &TableId) -> EtlResult<Option<Arc<TableSchema>>> {
        let inner = self.inner.lock().await;
        Ok(inner.table_schemas.get(table_id).cloned())
    }

    async fn get_table_schemas(&self) -> EtlResult<Vec<Arc<TableSchema>>> {
        let inner = self.inner.lock().await;
        Ok(inner.table_schemas.values().cloned().collect())
    }

    async fn load_table_schemas(&self) -> EtlResult<usize> {
        debug!("loading table schemas from SQLite store");

        let schemas = queries::load_schemas(&self.pool, self.pipeline_id as i64).await?;

        let count = schemas.len();
        let mut inner = self.inner.lock().await;
        inner.table_schemas.clear();
        for (id, schema) in schemas {
            inner.table_schemas.insert(id, Arc::new(schema));
        }

        info!(count, "loaded table schemas from SQLite store");
        Ok(count)
    }

    async fn store_table_schema(&self, table_schema: TableSchema) -> EtlResult<()> {
        let pid = self.pipeline_id as i64;

        queries::upsert_schema(&self.pool, pid, &table_schema).await?;

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

impl CleanupStore for SqliteStore {
    async fn cleanup_table_state(&self, table_id: TableId) -> EtlResult<()> {
        let pid = self.pipeline_id as i64;
        let tid = table_id.into_inner() as i64;

        queries::cleanup_table(&self.pool, pid, tid).await?;

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
    use etl_postgres::{
        replication::schema::string_to_postgres_type,
        types::{ColumnSchema, TableName},
    };

    use super::*;

    async fn test_store() -> SqliteStore {
        SqliteStore::new(1, ":memory:")
            .await
            .expect("failed to create test store")
    }

    #[tokio::test]
    async fn test_update_and_get_replication_state() {
        let store = test_store().await;
        let table_id = TableId::new(42);

        store
            .update_table_replication_state(table_id, TableReplicationPhase::Init)
            .await
            .unwrap();

        let state = store.get_table_replication_state(table_id).await.unwrap();
        assert_eq!(state, Some(TableReplicationPhase::Init));
    }

    #[tokio::test]
    async fn test_rollback_state() {
        let store = test_store().await;
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

    #[tokio::test]
    async fn test_rollback_no_previous_state_errors() {
        let store = test_store().await;
        let table_id = TableId::new(30);

        store
            .update_table_replication_state(table_id, TableReplicationPhase::Init)
            .await
            .unwrap();

        let result = store.rollback_table_replication_state(table_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_persists_states() {
        let store = test_store().await;
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

        // Clear cache, then load from SQLite
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

    #[tokio::test]
    async fn test_load_table_mappings_from_db() {
        let store = test_store().await;
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

        // Clear cache then reload
        {
            let mut inner = store.inner.lock().await;
            inner.table_mappings.clear();
        }

        let count = store.load_table_mappings().await.unwrap();
        assert_eq!(count, 2);

        assert_eq!(store.get_table_mapping(&t1).await.unwrap(), Some("dest_a".to_string()));
    }

    #[tokio::test]
    async fn test_cleanup_table_state() {
        let store = test_store().await;
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

    #[tokio::test]
    async fn test_load_table_schemas_from_db() {
        let store = test_store().await;
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

        // Clear cache then reload
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
