use std::collections::{BTreeMap, HashMap};

use etl::{
    error::{ErrorKind, EtlResult},
    etl_error,
    state::table::TableReplicationPhase,
};
use etl_postgres::{
    replication::{
        schema::{postgres_type_to_string, string_to_postgres_type},
        state::TableReplicationState,
    },
    types::{ColumnSchema, TableId, TableName, TableSchema},
};
use sqlx::SqlitePool;

use super::types::{MappingRow, SchemaColumnRow, StateRow};

pub async fn load_replication_states(pool: &SqlitePool, pid: i64) -> EtlResult<BTreeMap<TableId, TableReplicationPhase>> {
    let rows: Vec<StateRow> = sqlx::query_as("SELECT id, table_id, metadata, prev FROM replication_state WHERE pipeline_id = ?1 AND is_current = 1")
        .bind(pid)
        .fetch_all(pool)
        .await?;

    let mut states = BTreeMap::new();
    for row in rows {
        let table_id =
            TableId::new(u32::try_from(row.table_id).map_err(|_| etl_error!(ErrorKind::DeserializationError, "table_id out of u32 range", row.table_id))?);
        let phase = TableReplicationPhase::try_from(row)?;
        states.insert(table_id, phase);
    }

    Ok(states)
}

pub async fn upsert_replication_state(pool: &SqlitePool, pid: i64, tid: i64, phase: &TableReplicationPhase) -> EtlResult<()> {
    let db_state: TableReplicationState = phase.clone().try_into()?;
    let (state_type, metadata) = db_state
        .to_storage_format()
        .map_err(|e| etl_error!(ErrorKind::SerializationError, "State serialization failed", e.to_string()))?;

    let mut tx = pool.begin().await?;

    let current_id: Option<i64> = sqlx::query_scalar("SELECT id FROM replication_state WHERE pipeline_id = ?1 AND table_id = ?2 AND is_current = 1")
        .bind(pid)
        .bind(tid)
        .fetch_optional(&mut *tx)
        .await?;

    if let Some(prev_id) = current_id {
        sqlx::query("UPDATE replication_state SET is_current = 0, updated_at = datetime('now') WHERE id = ?1")
            .bind(prev_id)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query("INSERT INTO replication_state (pipeline_id, table_id, state, metadata, prev, is_current) VALUES (?1, ?2, ?3, ?4, ?5, 1)")
        .bind(pid)
        .bind(tid)
        .bind(format!("{state_type:?}"))
        .bind(metadata)
        .bind(current_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn rollback_replication_state(pool: &SqlitePool, pid: i64, tid: i64) -> EtlResult<TableReplicationPhase> {
    let mut tx = pool.begin().await?;

    let current: StateRow =
        sqlx::query_as("SELECT id, table_id, metadata, prev FROM replication_state WHERE pipeline_id = ?1 AND table_id = ?2 AND is_current = 1")
            .bind(pid)
            .bind(tid)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| etl_error!(ErrorKind::StateRollbackError, "No current state found for rollback"))?;

    let prev_id = current
        .prev
        .ok_or_else(|| etl_error!(ErrorKind::StateRollbackError, "No previous state available to roll back to"))?;

    sqlx::query("DELETE FROM replication_state WHERE id = ?1")
        .bind(current.id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("UPDATE replication_state SET is_current = 1, updated_at = datetime('now') WHERE id = ?1")
        .bind(prev_id)
        .execute(&mut *tx)
        .await?;

    let restored: StateRow = sqlx::query_as("SELECT id, table_id, metadata, prev FROM replication_state WHERE id = ?1")
        .bind(prev_id)
        .fetch_one(&mut *tx)
        .await?;

    tx.commit().await?;
    TableReplicationPhase::try_from(restored)
}

pub async fn load_mappings(pool: &SqlitePool, pid: i64) -> EtlResult<HashMap<TableId, String>> {
    let rows: Vec<MappingRow> = sqlx::query_as("SELECT source_table_id, destination_table_id FROM table_mappings WHERE pipeline_id = ?1")
        .bind(pid)
        .fetch_all(pool)
        .await?;

    let mut mappings = HashMap::new();
    for row in rows {
        let table_id = TableId::new(u32::try_from(row.source_table_id).map_err(|_| {
            etl_error!(
                ErrorKind::DeserializationError,
                "source_table_id out of u32 range",
                row.source_table_id
            )
        })?);
        mappings.insert(table_id, row.destination_table_id);
    }

    Ok(mappings)
}

pub async fn upsert_mapping(pool: &SqlitePool, pid: i64, sid: i64, dest: &str) -> EtlResult<()> {
    sqlx::query(
        "INSERT INTO table_mappings (pipeline_id, source_table_id, destination_table_id) VALUES (?1, ?2, ?3)
         ON CONFLICT (pipeline_id, source_table_id) DO UPDATE SET destination_table_id = excluded.destination_table_id, updated_at = datetime('now')",
    )
    .bind(pid)
    .bind(sid)
    .bind(dest)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn load_schemas(pool: &SqlitePool, pid: i64) -> EtlResult<HashMap<TableId, TableSchema>> {
    let rows: Vec<SchemaColumnRow> = sqlx::query_as(
        r#"
        SELECT
            ts.table_id, ts.schema_name, ts.table_name,
            tc.column_name, tc.column_type, tc.type_modifier,
            tc.nullable, tc.primary_key
        FROM table_schemas ts
        INNER JOIN table_columns tc ON ts.id = tc.table_schema_id
        WHERE ts.pipeline_id = ?1
        ORDER BY ts.table_id, tc.column_order
        "#,
    )
    .bind(pid)
    .fetch_all(pool)
    .await?;

    let mut schemas: HashMap<u32, TableSchema> = HashMap::new();

    for row in rows {
        let tid = u32::try_from(row.table_id).map_err(|_| etl_error!(ErrorKind::DeserializationError, "table_id out of u32 range", row.table_id))?;
        let table_id = TableId::new(tid);
        let entry = schemas.entry(tid).or_insert_with(|| {
            TableSchema::new(
                table_id,
                TableName::new(row.schema_name.clone(), row.table_name.clone()),
                vec![],
            )
        });

        entry.add_column_schema(ColumnSchema::new(
            row.column_name,
            string_to_postgres_type(&row.column_type),
            row.type_modifier as i32,
            row.nullable != 0,
            row.primary_key != 0,
        ));
    }

    Ok(schemas.into_values().map(|v| (v.id, v)).collect())
}

pub async fn upsert_schema(pool: &SqlitePool, pid: i64, schema: &TableSchema) -> EtlResult<()> {
    let tid = schema.id.into_inner() as i64;

    let mut tx = pool.begin().await?;

    let schema_id: i64 = sqlx::query_scalar(
        "INSERT INTO table_schemas (pipeline_id, table_id, schema_name, table_name) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (pipeline_id, table_id) DO UPDATE SET schema_name = excluded.schema_name, table_name = excluded.table_name, updated_at = datetime('now')
         RETURNING id",
    )
    .bind(pid)
    .bind(tid)
    .bind(&schema.name.schema)
    .bind(&schema.name.name)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM table_columns WHERE table_schema_id = ?1")
        .bind(schema_id)
        .execute(&mut *tx)
        .await?;

    for (order, col) in schema.column_schemas.iter().enumerate() {
        let col_type_str = postgres_type_to_string(&col.typ);
        sqlx::query(
            "INSERT INTO table_columns (table_schema_id, column_name, column_type, type_modifier, nullable, primary_key, column_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(schema_id)
        .bind(&col.name)
        .bind(col_type_str)
        .bind(col.modifier as i64)
        .bind(col.nullable as i64)
        .bind(col.primary as i64)
        .bind(order as i64)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn cleanup_table(pool: &SqlitePool, pid: i64, tid: i64) -> EtlResult<()> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM table_mappings WHERE pipeline_id = ?1 AND source_table_id = ?2")
        .bind(pid)
        .bind(tid)
        .execute(&mut *tx)
        .await?;

    // CASCADE handles table_columns deletion automatically
    sqlx::query("DELETE FROM table_schemas WHERE pipeline_id = ?1 AND table_id = ?2")
        .bind(pid)
        .bind(tid)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM replication_state WHERE pipeline_id = ?1 AND table_id = ?2")
        .bind(pid)
        .bind(tid)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}
