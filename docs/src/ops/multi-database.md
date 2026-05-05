# Multi-Database Setup

A single pgsense-rs process can monitor multiple PostgreSQL databases
concurrently. Each `[[databases]]` entry runs an independent pipeline
with its own replication slot, scan filter, and metrics labels.

## Why one process

- One alert channel pool — all findings flow through the same dispatcher
  with shared dedup and rate limits.
- One set of detection rules — keep PII policy consistent across
  databases without duplicating config.
- One Prometheus scrape target — fewer endpoints, single dashboard.
- Lower per-instance overhead than running N separate processes.

> [!WARNING]
> The trade-off is **shared blast radius**: if the process crashes,
> every monitored database goes offline at the same time. If isolation
> matters more than consolidation, run one pgsense-rs deployment per
> database instead.

## Example

```toml
rules_file = "config/rules.toml"

# Default scan filter — applies to any database without its own [databases.scan]
[scan]
exclude_columns = ["*_hash", "*_token"]

# --- Production primary ---
[[databases]]
host          = "primary.db.svc.cluster.local"
dbname        = "orders"
username      = "pgsense"
password       = { file = "/run/secrets/orders-pw" }
publication   = "pgsense_pub"

[databases.scan]
include_schemas = ["public", "billing"]

# --- Analytics replica ---
[[databases]]
host          = "analytics.db.svc.cluster.local"
dbname        = "events"
username      = "pgsense"
password       = { file = "/run/secrets/events-pw" }
publication   = "pgsense_pub"

[databases.scan]
include_schemas = ["events_*"]
exclude_tables  = ["events_raw"]
```

## Identity and metrics

Each database is identified by `"{host}/{dbname}"`. This string is the
value of the `database` label on every metric:

```
pgsense_events_total{database="primary.db.svc.cluster.local/orders"}
pgsense_events_total{database="analytics.db.svc.cluster.local/events"}
```

Pipelines, dedup keys, and Prometheus series are all keyed by this
identity. Two `[[databases]]` entries with the same identity are
rejected at startup.

## Independent reconnection

Each pipeline reconnects independently. A single database going down
doesn't take the others with it; the
`pgsense_pipeline_connected{database}` gauge tells you which is up at
any moment, and `pgsense_pipeline_reconnects_total{database}` counts
reconnect attempts per database.

## Per-rule routing across databases

Rules apply to *every* database. Use [scope](../rules/scope.md) on the
rule to limit it to specific schemas, tables, or columns (which can be
database-specific by name), and use the `channels` field to route
findings to specific alert destinations.

## Replication slot accounting

Each database needs one replication slot on its source server. Make
sure `max_replication_slots` is sized for the total fleet of pgsense-rs
deployments hitting that server.

> [!TIP]
> Run `pgsense-rs validate -c config.toml --connect` after editing the
> config — it confirms each database is reachable, the publication
> exists, and credentials work. Catches typos and permission issues
> before the supervisor tries to spawn pipelines.
