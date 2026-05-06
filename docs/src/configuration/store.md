# State Store

pgsense-rs keeps replication progress (LSN checkpoints, table-sync state) in
a store. The choice between in-memory and PostgreSQL-backed storage is the
single setting `store` under the `[pipeline]` section:

```toml
[pipeline]
store = "memory"     # default — state is lost on restart
# or
store = "postgres"   # state persists in the source DB under the `etl` schema
```

## Memory

In-memory state is the default and requires no setup.

> [!WARNING]
> On every restart, the pipeline starts from PostgreSQL's current LSN and
> may miss events written during the restart window. Use `memory` only for
> local development and ephemeral environments.

## PostgreSQL

When `store = "postgres"`, state is stored in **the source database itself**
under a hardcoded `etl` schema, using the same credentials as the
replication connection. The scanner installs the schema and bootstrap
helper functions on startup if they don't exist.

This means:

- Restarts resume from the last persisted LSN — no missed events.
- State and source data are backed up together, so they stay consistent.
- The replication user must be able to `CREATE SCHEMA` (or the `etl`
  schema must already exist with appropriate ownership).
- First-time migration application requires **superuser** because one
  of the upstream migrations creates an event trigger
  (`supabase_etl_ddl_message_trigger`). After migrations apply, the
  role can be downgraded. See [PostgreSQL Setup → `etl` schema
  bootstrap](../getting-started/postgres-setup.md#etl-schema-bootstrap)
  for the full permission matrix.

> [!NOTE]
> There is no separate state-database option — state always lives in the
> source database. This is a deliberate trade-off; see the project
> [README](https://github.com/MrEhbr/pgsense-rs#readme) for the reasoning.

## Switching stores

> [!CAUTION]
> The two stores are not interchangeable at runtime. Switching from
> `memory` to `postgres` (or back) on an existing deployment effectively
> restarts the pipeline from scratch — there is no migration path.
