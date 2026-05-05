# PostgreSQL Channel

Inserts each finding into a target PostgreSQL table. Useful for SIEM
ingestion, custom dashboards, or correlation with other database
metadata. Only one PostgreSQL alert channel can be configured.

## Configuration

```toml
[alerts.postgres]
name          = "postgres"          # optional — default "postgres"
host          = "localhost"         # default "localhost"
port          = 5432                # default 5432
dbname        = "postgres"          # default "postgres"
username      = "postgres"          # default "postgres"
password      = "..."               # inline value, or:
# password    = { file = "/run/secrets/alerts-pw" }
schema        = "pgsense"           # default "pgsense"
table         = "findings"          # default "findings"

[alerts.postgres.tls]
enabled = false
```

> [!IMPORTANT]
> Both `schema` and `table` must be valid PostgreSQL identifiers
> (ASCII letters, digits, underscores only — no hyphens, dots, quotes,
> or spaces). Invalid identifiers are rejected at startup.

## Auto-created schema

The scanner creates the schema and table on startup with
`CREATE SCHEMA IF NOT EXISTS` and `CREATE TABLE IF NOT EXISTS`. The
schema is exactly:

```sql
CREATE TABLE "<schema>"."<table>" (
    id            BIGSERIAL PRIMARY KEY,
    database      TEXT NOT NULL,
    rule_id       TEXT NOT NULL,
    description   TEXT NOT NULL,
    category      TEXT NOT NULL,
    severity      TEXT NOT NULL,
    schema_name   TEXT NOT NULL,
    table_name    TEXT NOT NULL,
    column_name   TEXT NOT NULL,
    masked_sample TEXT NOT NULL,
    primary_key   JSONB NOT NULL DEFAULT '{}',
    lsn           BIGINT NOT NULL,
    detected_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

> [!NOTE]
> The username configured here needs `CREATE` on the database (for the
> schema) and `CREATE` on the schema (for the table) on first startup,
> then `INSERT` on the table for ongoing operation. After the first run
> you can revoke `CREATE` if your security policy requires it.

The connection pool is small (max 2 connections) and sets `search_path`
to the configured schema on every checkout.

## Why a separate database

> [!WARNING]
> The alert channel typically points at a **different** database from
> the one being scanned. Writing findings into the source database can
> create a detection loop — pgsense-rs would scan its own findings
> table and surface new findings whose values are masked replicas of
> older ones.

If you must use the same database, exclude the target table via the
scan filter:

```toml
[scan]
exclude_tables = ["findings"]
```

## Failure handling

Failed inserts are logged at `error` and counted in
`pgsense_alerts_total{channel="postgres",status="error"}`. The scanner
does not buffer failed findings on disk — connection or insert failures
result in dropped findings.

## Use cases

- Centralized findings store across many pgsense-rs instances.
- BI / dashboard backend (Grafana datasource, Metabase, etc.).
- Correlation with audit logs or schema metadata in the same Postgres.
