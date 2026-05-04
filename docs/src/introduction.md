# Introduction

**pgsense-rs** is a rule-based service that monitors PostgreSQL logical
replication streams to detect sensitive data and trigger real-time alerts.

## Why pgsense-rs?

Sensitive data — credit card numbers, SSNs, email addresses, API keys —
ends up in production databases through application bugs, unvalidated
imports, or missing input filters. By the time you notice, the data is
already persisted and potentially exposed.

pgsense-rs watches the PostgreSQL WAL in real time, catching sensitive
values **as they are written** rather than after the fact. It runs as a
separate process with zero changes to your application code or schema.

## How It Works

```
PostgreSQL WAL → etl Pipeline → Scanner → Rule Engine → Alert Dispatcher
```

1. **Pipeline** — Connects to PostgreSQL's [logical replication stream](https://www.postgresql.org/docs/current/logical-replication.html) via [supabase/etl](https://github.com/supabase/etl), batches change events, and forwards them through an internal channel.
2. **Scanner** — Filters out non-text columns (integers, booleans, timestamps, UUIDs, bytea, etc.) to reduce noise, then passes text values to the rule engine.
3. **Rule Engine** — Three-phase detection:
   - **Regex rules** — fast-path filtering through a combined regex set, then individual regex match + optional validator (Luhn, SSN, phone, email, IBAN).
   - **Builtin detectors** — algorithmic scanning with boundary-aware matching (credit cards, IBANs, SSNs, phone numbers, email addresses).
   - **Rhai scripts** — custom detection logic in sandboxed scripts.
4. **Alert Dispatcher** — Deduplicates findings within a configurable window, then routes to named alert channels.

> [!NOTE]
> pgsense-rs reads the WAL through PostgreSQL's standard logical
> replication protocol — no triggers, no rewrites, no read traffic on
> the source tables. The scanner appears to PostgreSQL as one
> additional replication subscriber.

## Features

- **Real-time scanning** of INSERT and UPDATE events from the PostgreSQL WAL
- **Three rule types** — regex with validators, builtin algorithmic detectors, Rhai scripts
- **Hot reload** — edit the rules file and changes take effect without restart
- **Deduplication** — same `(database, schema, table, column, rule, value)` finding is suppressed within a configurable window
- **Multiple alert channels** — log, stdout, JSONL file, webhook, Slack, PostgreSQL — with optional per-rule routing
- **Prometheus metrics** — events processed, findings by category and severity, alert delivery, scan latency
- **Health endpoints** — `/health`, `/ready`, `/metrics` via configurable HTTP server
- **Multi-database** — monitor multiple PostgreSQL databases concurrently with independent pipelines, scan filters, and metrics labels
- **Per-rule scope** — restrict rules to specific schemas, tables, or columns
- **Allowlists** — per-rule value and pattern allowlists to reduce false positives
- **Value masking** — matched values are masked in alert output

## Where to Go Next

- [Installation](./getting-started/installation.md) — get a binary
- [Quick Start](./getting-started/quick-start.md) — first scan in five minutes
- [PostgreSQL Setup](./getting-started/postgres-setup.md) — server-side prerequisites
- [Configuration](./configuration/index.md) — full configuration reference
- [Detection Rules](./rules/index.md) — write your own rules
- [Alert Channels](./alerts/index.md) — wire up notifications
