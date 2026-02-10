-- Base schema for pgsense pipeline store.
-- Adapted from etl-replicator's base migration.
-- No CREATE SCHEMA — schema is set via search_path on the connection.

-- Replication state
create table if not exists replication_state (
    id bigint generated always as identity primary key,
    pipeline_id bigint not null,
    table_id oid not null,
    state text not null,
    metadata jsonb,
    prev bigint references replication_state(id),
    is_current boolean not null default true,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- Ensures that there is only one current state per pipeline/table
create unique index if not exists uq_replication_state_current_true
    on replication_state (pipeline_id, table_id)
    where is_current = true;

-- Table schemas (per pipeline, per table)
create table if not exists table_schemas (
    id bigint generated always as identity primary key,
    pipeline_id bigint not null,
    table_id oid not null,
    schema_name text not null,
    table_name text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (pipeline_id, table_id)
);

create index if not exists idx_table_schemas_pipeline_table
    on table_schemas (pipeline_id, table_id);

-- Columns for stored schemas
create table if not exists table_columns (
    id bigint generated always as identity primary key,
    table_schema_id bigint not null references table_schemas(id) on delete cascade,
    column_name text not null,
    column_type text not null,
    type_modifier integer not null,
    nullable boolean not null,
    primary_key boolean not null,
    column_order integer not null,
    created_at timestamptz not null default now(),
    unique (table_schema_id, column_name),
    unique (table_schema_id, column_order)
);

create index if not exists idx_table_columns_order
    on table_columns (table_schema_id);

-- Source-to-destination table id mappings
create table if not exists table_mappings (
    id bigint generated always as identity primary key,
    pipeline_id bigint not null,
    source_table_id oid not null,
    destination_table_id text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (pipeline_id, source_table_id)
);
