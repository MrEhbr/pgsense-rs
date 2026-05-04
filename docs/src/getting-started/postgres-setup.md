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

Create a dedicated role with the minimum privileges required:

```sql
CREATE ROLE pgsense WITH LOGIN REPLICATION PASSWORD '...';
GRANT CONNECT ON DATABASE your_db TO pgsense;
GRANT USAGE ON SCHEMA public TO pgsense;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO pgsense;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO pgsense;
```

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

## State storage

When `[pipeline] store = "postgres"`, the scanner stores replication
state in the source database under a hardcoded `etl` schema, using the
same credentials as the replication connection. Bootstrap helper
functions are installed automatically on pipeline startup.

For this to work, the role needs `CREATE` on the database (to create
the schema on first run):

```sql
GRANT CREATE ON DATABASE your_db TO pgsense;
```

After the first run you can revoke `CREATE` if your security policy
requires it — the scanner only needs `INSERT/UPDATE/SELECT` on tables
inside the `etl` schema for ongoing operation.

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
