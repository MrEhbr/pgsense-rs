# PostgreSQL Setup

pgsense-rs consumes PostgreSQL's logical replication stream. This page
covers the server-side configuration required for the scanner to
connect and stay connected.

## Server requirements

- PostgreSQL **16 or newer** (the `pubviaroot` column on `pg_publication` is required)
- `wal_level = logical` in `postgresql.conf`
- Sufficient `max_replication_slots` and `max_wal_senders` for one slot
  per scanned database

## postgresql.conf

```
wal_level = logical
max_replication_slots = 10        # at least 1 per pgsense-rs database
max_wal_senders       = 10
```

> [!IMPORTANT]
> `wal_level` and the slot/sender limits all require a server restart
> to take effect.

## Replication role

Create a dedicated role with the minimum privileges required for the
replication stream itself:

```sql
CREATE ROLE pgsense WITH LOGIN REPLICATION PASSWORD '...';
GRANT CONNECT ON DATABASE your_db TO pgsense;
GRANT USAGE ON SCHEMA public TO pgsense;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO pgsense;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO pgsense;
```

If the publication includes tables in schemas other than `public`,
repeat the `USAGE`, `SELECT`, and `ALTER DEFAULT PRIVILEGES` grants for
each of those schemas.

The `REPLICATION` attribute is what authorizes the runtime SQL the
scanner issues against the WAL sender: `IDENTIFY_SYSTEM`,
`CREATE_REPLICATION_SLOT … LOGICAL pgoutput`, `START_REPLICATION SLOT
… LOGICAL`, the initial-snapshot `COPY … TO STDOUT` for each table,
and `pg_export_snapshot()`.

## Publication

Create one publication per database that pgsense-rs should monitor:

```sql
-- All tables
CREATE PUBLICATION pgsense_pub FOR ALL TABLES;

-- Specific tables only
CREATE PUBLICATION pgsense_pub FOR TABLE public.users, public.orders;
```

The publication name must match the `publication` field on the
corresponding `[[databases]]` entry in your config (default
`pgsense_pub`).

## `etl` schema bootstrap

Regardless of which state store you pick, pgsense-rs installs a small
amount of SQL into a hardcoded `etl` schema in the source database on
pipeline startup. This is required by the upstream replication library
to describe table schemas during table sync. The exact set of objects
depends on `[pipeline] store`.

### `store = "memory"` (default)

On every PipelineRunner startup, pgsense-rs applies an idempotent
bootstrap (`create or replace function`, `create schema if not
exists`):

- `etl` schema
- `etl.describe_table_schema(oid)` SQL function
- `etl.describe_table_identity(oid)` SQL function

The role therefore needs `CREATE` on the database for the very first
run:

```sql
GRANT CREATE ON DATABASE your_db TO pgsense;
```

After the first run you can revoke it again — subsequent runs only
re-issue `create or replace` against existing objects, which only
needs ownership of the `etl` schema.

> [!NOTE]
> The Memory store deliberately does **not** install the
> `supabase_etl_ddl_message_trigger` event trigger that the upstream
> migrations would normally add. As a result, `ALTER TABLE` on a
> published table during streaming makes the apply worker fail once;
> the supervisor reconnects and resumes from the slot, so recovery is
> automatic but produces an error log and ~1s gap per `ALTER TABLE`.
> Use the Postgres store if you experience frequent schema changes.

### `store = "postgres"`

The scanner runs a small set of versioned SQL migrations (vendored
from the upstream `etl` project at the pinned revision, currently
[`etl @ ce88ba7`][etl-migrations]) to set up the `etl` schema on the
source database. State (replication progress, per-table sync state,
cached table schemas) is persisted there.

[etl-migrations]: https://github.com/supabase/etl/tree/ce88ba7/etl/migrations

The migrations create:

- The `etl` schema and a `_sqlx_migrations` table tracking which
  migrations have been applied
- Enums: `etl.table_state`, `etl.destination_table_schema_status`
- Tables: `etl.replication_state`, `etl.table_schemas`,
  `etl.table_columns`, `etl.destination_tables_metadata`
  (plus indexes and unique constraints)
- The `etl.describe_table_schema(oid)` and
  `etl.describe_table_identity(oid)` functions
- The `etl.emit_schema_change_messages()` plpgsql function
  (`security definer`)
- The `supabase_etl_ddl_message_trigger` event trigger on
  `ddl_command_end` for `ALTER TABLE`

`supabase_etl_ddl_message_trigger` fires on `ALTER TABLE` against
published tables and emits a logical message into the WAL describing
the new column layout. The apply worker reads it from the slot and
refreshes its cached schema before the next DML, so column changes
don't cause an error/reconnect cycle (compare the Memory-store path,
which has no trigger and recovers via reconnect once per
`ALTER TABLE`).

> [!IMPORTANT]
> Only the third migration
> (`20260415100000_schema_change_messages.sql`) needs **superuser**,
> because PostgreSQL restricts `CREATE EVENT TRIGGER` to superusers.
> The first run with `store = "postgres"` must apply migrations as a
> superuser. Once applied, the trigger persists and fires under any
> role; the superuser is only needed to *create* it, not to run it.
> Steady-state operation needs only `CREATE` on the database (for the
> idempotent `create schema if not exists`), `USAGE` on the `etl`
> schema, and `SELECT/INSERT/UPDATE/DELETE` on its tables.

The simplest path is to start pgsense-rs once with a superuser role,
let it apply migrations, then downgrade the role. pgsense-rs records
each applied migration in `etl._sqlx_migrations`; subsequent startups
under the downgraded role see all migrations as already applied and
skip them.

If running pgsense-rs as a superuser even once is unacceptable, an
operator can apply the migrations directly under a superuser session
and seed `etl._sqlx_migrations` so the next pgsense-rs startup
considers them applied. The migration files for the pinned revision
are at [`etl/migrations/`][etl-migrations]. Each row's `checksum` is
the SHA-384 of the corresponding migration file's bytes; a startup
will refuse to proceed if the stored checksum disagrees with the
file. Values below match the pinned revision:

```sql
-- Apply the three migrations as a superuser first (in order):
--   etl/migrations/20250827000000_base.sql
--   etl/migrations/20260415090000_schema_storage_ddl_support.sql
--   etl/migrations/20260415100000_schema_change_messages.sql
-- Then create the migration tracker table and seed it:

CREATE TABLE IF NOT EXISTS etl._sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),
    success BOOLEAN NOT NULL,
    checksum BYTEA NOT NULL,
    execution_time BIGINT NOT NULL
);

INSERT INTO etl._sqlx_migrations
    (version, description, success, checksum, execution_time)
VALUES
    (20250827000000, 'base', true,
     '\xf9a851cc59b9777ef680c271ba52f060da848b31fa6cd77123b6205f9dcd99ff332981954dcadda62a784ce7aed1e881',
     0),
    (20260415090000, 'schema storage ddl support', true,
     '\x23248be3ed9b589315ca3b5eb693296f97f50a48e719212c432c7a9b20d4c7a3a67a32b66d45dd0cf194ae87f712efd0',
     0),
    (20260415100000, 'schema change messages', true,
     '\x8f3ca7fc2c72d4d7beaf08f0d9df7c56f13f4d7ecfee61653820224d8b1b106a62117070898986bc5abc6dd17e38c6cb',
     0)
ON CONFLICT (version) DO NOTHING;
```

To verify a checksum locally:

```bash
shasum -a 384 etl/migrations/20250827000000_base.sql
```

After this, pgsense-rs can run as a non-superuser indefinitely.

> [!NOTE]
> When the etl revision pinned by pgsense-rs is bumped to a version
> that adds new migrations, the same superuser-required step has to
> happen again on the next deployment. We will call this out in
> release notes when it occurs.

> [!CAUTION]
> Cloud-managed Postgres (RDS, Cloud SQL, Azure, Supabase) does **not**
> grant true superuser to user roles, which means
> `store = "postgres"` cannot install the event trigger out of the
> box. Stick with `store = "memory"` on managed services unless the
> provider exposes an event-trigger-capable role
> (e.g. `rds_superuser` does **not** suffice — only on-instance
> superuser does).

## Cloud PostgreSQL notes

> [!WARNING]
> Cloud Postgres providers each require a different parameter to enable
> logical replication. Changing the parameter usually requires an
> instance restart, not just a parameter-group apply.

- **AWS RDS / Aurora** — Set `rds.logical_replication = 1` in the parameter group, reboot, then create the publication. Grant `rds_replication` to the replication role.
- **GCP Cloud SQL** — Enable the `cloudsql.logical_decoding` flag, restart, then create the publication.
- **Azure Database for PostgreSQL** — Set `wal_level = logical` via the server parameters blade, restart, then create the publication.
- **Supabase / Neon** — Logical replication is enabled by default; create the publication directly.

## Replication slot hygiene

> [!CAUTION]
> A replication slot keeps WAL segments on disk until the subscriber
> consumes them. If pgsense-rs is shut down and the slot is left in
> place (e.g. because you scaled the deployment to zero), WAL on the
> source server will accumulate indefinitely, eventually filling the
> disk. Drop the slot with `pg_drop_replication_slot('<slot_name>')`
> if you decommission a pgsense-rs deployment.

Slot names are derived deterministically from `host/dbname`, so
restarting pgsense-rs against the same database resumes the existing
slot rather than orphaning it.
