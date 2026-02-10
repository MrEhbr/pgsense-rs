-- SQLite-adapted schema matching etl's PostgresStore base migration.
-- Key differences from Postgres: AUTOINCREMENT vs GENERATED ALWAYS AS IDENTITY,
-- INTEGER for booleans, TEXT for timestamps, no schema prefix.

CREATE TABLE IF NOT EXISTS replication_state (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    pipeline_id INTEGER NOT NULL,
    table_id    INTEGER NOT NULL,
    state       TEXT NOT NULL,
    metadata    TEXT,
    prev        INTEGER REFERENCES replication_state(id),
    is_current  INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_replication_state_current_true
    ON replication_state (pipeline_id, table_id) WHERE is_current = 1;

CREATE TABLE IF NOT EXISTS table_schemas (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    pipeline_id INTEGER NOT NULL,
    table_id    INTEGER NOT NULL,
    schema_name TEXT NOT NULL,
    table_name  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (pipeline_id, table_id)
);

CREATE INDEX IF NOT EXISTS idx_table_schemas_pipeline_table
    ON table_schemas (pipeline_id, table_id);

CREATE TABLE IF NOT EXISTS table_columns (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    table_schema_id INTEGER NOT NULL REFERENCES table_schemas(id) ON DELETE CASCADE,
    column_name     TEXT NOT NULL,
    column_type     TEXT NOT NULL,
    type_modifier   INTEGER NOT NULL,
    nullable        INTEGER NOT NULL,
    primary_key     INTEGER NOT NULL,
    column_order    INTEGER NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (table_schema_id, column_name),
    UNIQUE (table_schema_id, column_order)
);

CREATE INDEX IF NOT EXISTS idx_table_columns_order
    ON table_columns (table_schema_id, column_order);

CREATE TABLE IF NOT EXISTS table_mappings (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    pipeline_id          INTEGER NOT NULL,
    source_table_id      INTEGER NOT NULL,
    destination_table_id TEXT NOT NULL,
    created_at           TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at           TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (pipeline_id, source_table_id)
);
