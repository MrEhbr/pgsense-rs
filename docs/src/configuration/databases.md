# Databases

Each `[[databases]]` block declares one PostgreSQL database for pgsense-rs to
monitor. The scanner runs an independent pipeline per entry, so you can
monitor multiple databases concurrently from a single process.

## Fields

```toml
[[databases]]
host          = "localhost"     # default "localhost"
port          = 5432            # default 5432
dbname        = "app"           # default "postgres"
username      = "pgsense"       # default "postgres"
password      = "..."           # or password_file
password_file = "/run/secrets/db-password"   # takes precedence over password
publication   = "pgsense_pub"   # default "pgsense_pub"

[databases.tls]
enabled            = false
trusted_root_certs = "/path/to/ca.pem"   # optional

[databases.scan]
# Per-database filter — overrides the top-level [scan] block.
# See the Scan Filter page for field details.
```

Validation rejects empty `host`, `dbname`, `username`, or `publication`,
and a `port` of 0.

## Identity key

Each database is identified by `"{host}/{dbname}"`. This string is used as:

- The `database` label on Prometheus metrics.
- The dedup key prefix.
- The replication slot ID (stable across restarts, derived from the identity
  string by a deterministic hash).

> [!IMPORTANT]
> Two `[[databases]]` entries with the same `host` and `dbname` are rejected
> at startup. The combination must be unique across all entries.

## Multiple databases

```toml
[[databases]]
host = "primary.example.com"
dbname = "orders"
username = "pgsense"
password_file = "/run/secrets/orders-pw"

[[databases]]
host = "secondary.example.com"
dbname = "users"
username = "pgsense"
password_file = "/run/secrets/users-pw"
```

Each database keeps its own replication slot, scan filter, and metrics
labels. Findings from all databases fan out to the same alert channels
unless a rule restricts itself with `channels = [...]`.

## See also

- [PostgreSQL Setup](../getting-started/postgres-setup.md) — server-side prerequisites
- [Scan Filter](./scan-filter.md) — narrow what each database scans
- [Multi-Database Setup](../ops/multi-database.md) — operational guidance
