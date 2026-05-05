# Configuration

pgsense-rs is configured through a TOML file, optionally overridden by
environment variables prefixed with `PGSENSE__`. The scanner ships with a
fully-commented example at [`config/config.toml`](https://github.com/MrEhbr/pgsense-rs/blob/main/config/config.toml).

## Top-level structure

```toml
# Detection rules file (also settable via --rules)
rules_file = "config/rules.toml"

# One or more PostgreSQL databases to monitor
[[databases]]
host        = "localhost"
port        = 5432
dbname      = "app"
username    = "pgsense"
password    = "..."           # or { file = "/run/secrets/pg_password" }
publication = "pgsense_pub"

# Optional per-database scan filter
[databases.scan]
include_schemas = ["public"]
exclude_tables  = ["audit_*", "tmp_*"]

# Default scan filter (applies to databases without their own [databases.scan])
[scan]
exclude_columns = ["*_hash", "updated_at"]

# Pipeline settings, including state store
[pipeline]
store = "memory"   # or "postgres" — see State Store

# Alerts (see Alert Channels)
[alerts.log]
enabled = true

# Optional HTTP server for /health, /ready, /metrics
[server]
enabled = false
port    = 9090
```

## Environment variable overrides

Every field has a matching env var with prefix `PGSENSE__` and double
underscores as the section separator:

```bash
PGSENSE__SERVER__PORT=9091
PGSENSE__DATABASES__0__PASSWORD=secret
PGSENSE__PIPELINE__STORE=postgres
```

> [!TIP]
> Env vars are the easiest way to inject non-secret configuration in
> container orchestrators that don't mount files. For secrets, prefer the
> file-backed form below — it avoids leaking values into process listings
> and child-process environments.

## Secrets

Every secret-bearing field accepts either an inline string or a file
reference:

```toml
# Inline (handy for local development)
password = "literal-value"

# File-backed (recommended for production)
password = { file = "/run/secrets/pg_password" }
```

The file's contents are read at startup with trailing whitespace stripped.
This shape applies to:

- `[[databases]].password`
- `[alerts.postgres].password`
- `[[alerts.slack]].token`
- `[[alerts.webhooks]].headers.<name>` (any header value)

> [!IMPORTANT]
> When deploying to Kubernetes, mount each `Secret` as a file and point
> the corresponding config field at it. This keeps plaintext credentials
> out of `ConfigMap` and out of process environments.

## Loading and validation

At startup, the scanner reads the TOML file, applies env overrides on
top, resolves file-backed secrets, and validates the result.
Invalid or missing fields fail fast at startup rather than at first
event. The standalone `validate` CLI subcommand runs the same checks
(plus optional live connectivity checks) without starting the scanner
— see [`pgsense-rs validate`](../cli/validate.md).

## Section reference

- [Databases](./databases.md) — connection details, publication, TLS
- [Scan Filter](./scan-filter.md) — schema/table/column include & exclude lists
- [State Store](./store.md) — memory vs PostgreSQL state persistence
- [Pipeline Tuning](./pipeline.md) — batch, retry, and worker-concurrency settings
- [Logging](./logging.md) — level, format, output target
- [Telemetry](./telemetry.md) — OTLP tracing exporter
- [Server](./server.md) — HTTP server for health and metrics endpoints
- [Profiling](./profiling.md) — per-rule and per-phase scan timing
