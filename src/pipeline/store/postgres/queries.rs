use std::collections::HashMap;

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
use sqlx::{PgExecutor, PgPool, Row, postgres::types::Oid as SqlxTableId};

use super::types::TableReplicationStateRow;

// Adapted from etl-postgres/src/replication/state.rs
pub async fn get_table_replication_state_rows(pool: &PgPool, pipeline_id: i64) -> EtlResult<Vec<TableReplicationStateRow>> {
    let rows: Vec<TableReplicationStateRow> = sqlx::query_as(
        r#"
        select id, pipeline_id, table_id, state, metadata, prev, is_current
        from replication_state
        where pipeline_id = $1 and is_current = true
        "#,
    )
    .bind(pipeline_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn update_replication_state(pool: &PgPool, pipeline_id: i64, table_id: TableId, phase: &TableReplicationPhase) -> EtlResult<()> {
    let db_state: TableReplicationState = phase.clone().try_into()?;
    let (state_type, metadata) = db_state
        .to_storage_format()
        .map_err(|e| etl_error!(ErrorKind::SerializationError, "State serialization failed", e.to_string()))?;

    let mut tx = pool.begin().await?;

    let current_id: Option<i64> = sqlx::query_scalar(
        r#"
        select id from replication_state
        where pipeline_id = $1 and table_id = $2 and is_current = true
        "#,
    )
    .bind(pipeline_id)
    .bind(SqlxTableId(table_id.into_inner()))
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(prev_id) = current_id {
        sqlx::query(
            r#"
            update replication_state
            set is_current = false, updated_at = now()
            where id = $1
            "#,
        )
        .bind(prev_id)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        r#"
        insert into replication_state (pipeline_id, table_id, state, metadata, prev, is_current)
        values ($1, $2, $3, $4, $5, true)
        "#,
    )
    .bind(pipeline_id)
    .bind(SqlxTableId(table_id.into_inner()))
    .bind(format!("{state_type:?}"))
    .bind(metadata)
    .bind(current_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn rollback_replication_state(pool: &PgPool, pipeline_id: i64, table_id: TableId) -> EtlResult<Option<TableReplicationStateRow>> {
    let mut tx = pool.begin().await?;

    let current_row: Option<(i64, Option<i64>)> = sqlx::query_as(
        r#"
        select id, prev from replication_state
        where pipeline_id = $1 and table_id = $2 and is_current = true
        "#,
    )
    .bind(pipeline_id)
    .bind(SqlxTableId(table_id.into_inner()))
    .fetch_optional(&mut *tx)
    .await?;

    if let Some((current_id, Some(prev_id))) = current_row {
        sqlx::query("delete from replication_state where id = $1")
            .bind(current_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            r#"
            update replication_state
            set is_current = true, updated_at = now()
            where id = $1
            "#,
        )
        .bind(prev_id)
        .execute(&mut *tx)
        .await?;

        let restored: TableReplicationStateRow = sqlx::query_as(
            r#"
            select id, pipeline_id, table_id, state, metadata, prev, is_current
            from replication_state
            where id = $1
            "#,
        )
        .bind(prev_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        return Ok(Some(restored));
    }

    tx.commit().await?;
    Ok(None)
}

pub async fn delete_replication_state_for_table<'c, E>(executor: E, pipeline_id: i64, table_id: TableId) -> sqlx::Result<u64>
where
    E: PgExecutor<'c>,
{
    let result = sqlx::query(
        r#"
        delete from replication_state
        where pipeline_id = $1 and table_id = $2
        "#,
    )
    .bind(pipeline_id)
    .bind(SqlxTableId(table_id.into_inner()))
    .execute(executor)
    .await?;

    Ok(result.rows_affected())
}

// Adapted from etl-postgres/src/replication/schema.rs
pub async fn store_table_schema(pool: &PgPool, pipeline_id: i64, table_schema: &TableSchema) -> EtlResult<()> {
    let mut tx = pool.begin().await?;

    let table_schema_id: i64 = sqlx::query(
        r#"
        insert into table_schemas (pipeline_id, table_id, schema_name, table_name)
        values ($1, $2, $3, $4)
        on conflict (pipeline_id, table_id)
        do update set
            schema_name = excluded.schema_name,
            table_name = excluded.table_name,
            updated_at = now()
        returning id
        "#,
    )
    .bind(pipeline_id)
    .bind(table_schema.id.into_inner() as i64)
    .bind(&table_schema.name.schema)
    .bind(&table_schema.name.name)
    .fetch_one(&mut *tx)
    .await?
    .get(0);

    sqlx::query("delete from table_columns where table_schema_id = $1")
        .bind(table_schema_id)
        .execute(&mut *tx)
        .await?;

    for (column_order, column_schema) in table_schema.column_schemas.iter().enumerate() {
        let column_type_str = postgres_type_to_string(&column_schema.typ);

        sqlx::query(
            r#"
            insert into table_columns
            (table_schema_id, column_name, column_type, type_modifier, nullable, primary_key, column_order)
            values ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(table_schema_id)
        .bind(&column_schema.name)
        .bind(column_type_str)
        .bind(column_schema.modifier)
        .bind(column_schema.nullable)
        .bind(column_schema.primary)
        .bind(column_order as i32)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn load_table_schemas(pool: &PgPool, pipeline_id: i64) -> EtlResult<Vec<TableSchema>> {
    let rows = sqlx::query(
        r#"
        select
            ts.table_id,
            ts.schema_name,
            ts.table_name,
            tc.column_name,
            tc.column_type,
            tc.type_modifier,
            tc.nullable,
            tc.primary_key,
            tc.column_order
        from table_schemas ts
        inner join table_columns tc on ts.id = tc.table_schema_id
        where ts.pipeline_id = $1
        order by ts.table_id, tc.column_order
        "#,
    )
    .bind(pipeline_id)
    .fetch_all(pool)
    .await?;

    let mut table_schemas: HashMap<TableId, TableSchema> = HashMap::new();

    for row in rows {
        let table_oid: SqlxTableId = row.get("table_id");
        let table_id = TableId::new(table_oid.0);
        let schema_name: String = row.get("schema_name");
        let table_name: String = row.get("table_name");

        let entry = table_schemas
            .entry(table_id)
            .or_insert_with(|| TableSchema::new(table_id, TableName::new(schema_name, table_name), vec![]));

        let column_name: String = row.get("column_name");
        let column_type: String = row.get("column_type");
        let type_modifier: i32 = row.get("type_modifier");
        let nullable: bool = row.get("nullable");
        let primary_key: bool = row.get("primary_key");

        entry.add_column_schema(ColumnSchema::new(
            column_name,
            string_to_postgres_type(&column_type),
            type_modifier,
            nullable,
            primary_key,
        ));
    }

    Ok(table_schemas.into_values().collect())
}

pub async fn delete_table_schema_for_table<'c, E>(executor: E, pipeline_id: i64, table_id: TableId) -> sqlx::Result<u64>
where
    E: PgExecutor<'c>,
{
    let result = sqlx::query(
        r#"
        delete from table_schemas
        where pipeline_id = $1 and table_id = $2
        "#,
    )
    .bind(pipeline_id)
    .bind(SqlxTableId(table_id.into_inner()))
    .execute(executor)
    .await?;

    Ok(result.rows_affected())
}

// Adapted from etl-postgres/src/replication/table_mappings.rs
pub async fn store_table_mapping(pool: &PgPool, pipeline_id: i64, source_table_id: TableId, destination_table_id: &str) -> EtlResult<()> {
    sqlx::query(
        r#"
        insert into table_mappings (pipeline_id, source_table_id, destination_table_id)
        values ($1, $2, $3)
        on conflict (pipeline_id, source_table_id)
        do update set
            destination_table_id = excluded.destination_table_id,
            updated_at = now()
        "#,
    )
    .bind(pipeline_id)
    .bind(SqlxTableId(source_table_id.into_inner()))
    .bind(destination_table_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn load_table_mappings(pool: &PgPool, pipeline_id: i64) -> EtlResult<HashMap<TableId, String>> {
    let rows = sqlx::query(
        r#"
        select source_table_id, destination_table_id
        from table_mappings
        where pipeline_id = $1
        "#,
    )
    .bind(pipeline_id)
    .fetch_all(pool)
    .await?;

    let mut mappings = HashMap::new();
    for row in rows {
        let source_table_id: SqlxTableId = row.get("source_table_id");
        let destination_table_id: String = row.get("destination_table_id");
        mappings.insert(TableId::new(source_table_id.0), destination_table_id);
    }

    Ok(mappings)
}

pub async fn delete_table_mappings_for_table<'c, E>(executor: E, pipeline_id: i64, source_table_id: TableId) -> sqlx::Result<u64>
where
    E: PgExecutor<'c>,
{
    let result = sqlx::query(
        r#"
        delete from table_mappings
        where pipeline_id = $1 and source_table_id = $2
        "#,
    )
    .bind(pipeline_id)
    .bind(SqlxTableId(source_table_id.into_inner()))
    .execute(executor)
    .await?;

    Ok(result.rows_affected())
}
