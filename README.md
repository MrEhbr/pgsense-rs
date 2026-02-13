# pgsense-rs

[![CI](https://github.com/MrEhbr/pgsense-rs/actions/workflows/checks.yml/badge.svg)](https://github.com/MrEhbr/pgsense-rs/actions)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024+-orange)](https://www.rust-lang.org)

A rule-based Rust service that monitors PostgreSQL logical replication streams to detect sensitive data and trigger real-time alerts.

## Why pgsense-rs?

Sensitive data — credit card numbers, SSNs, email addresses — ends up in production databases through application bugs, unvalidated imports, or missing input filters. By the time you notice, the data is already persisted and potentially exposed.

pgsense-rs watches the PostgreSQL WAL in real time, catching sensitive values **as they're written** rather than after the fact. It runs as a sidecar process with zero changes to your application code or schema.

## How It Works

```
PostgreSQL WAL → etl Pipeline → Scanner → Rule Engine → Alert Dispatcher
```

1. **Pipeline** — Connects to PostgreSQL's [logical replication stream](https://www.postgresql.org/docs/current/logical-replication.html) via [supabase/etl](https://github.com/supabase/etl), batches change events, and forwards them through an internal channel
2. **Scanner** — Filters out non-text columns (integers, booleans, timestamps, UUIDs, bytea) to reduce noise, then passes text values to the rule engine
3. **Rule Engine** — Three-phase detection:
   - **Regex rules** — RegexSet fast-path for bulk filtering, then individual regex match + optional validator (Luhn, SSN checksum)
   - **Builtin detectors** — Algorithmic scanning with boundary-aware matching (credit cards, SSNs)
   - **Rhai scripts** — Custom detection logic in sandboxed scripts
4. **Alert Dispatcher** — Deduplicates findings by (schema, table, column, rule, value) within a configurable window, then fans out to all enabled channels

## Features

- **Real-time scanning** of INSERT/UPDATE events from the PostgreSQL WAL
- **Three rule types**: regex with validators, builtin algorithmic detectors, Rhai scripts
- **Hot reload** — Edit the rules file and changes take effect without restart
- **Deduplication** — Same (schema, table, column, rule, value) finding is suppressed within a configurable window
- **Multiple alert channels** — Structured logging, stdout, JSONL file, webhooks, Slack (with batching), PostgreSQL table
- **Prometheus metrics** — Events processed, findings by category/severity, alert delivery status, scan latency
- **Health endpoints** — `/health`, `/ready`, `/metrics` via configurable HTTP server
- **Column-type filtering** — Automatically skips non-text column types
- **Persistent checkpointing** — Memory (default), SQLite, or PostgreSQL-backed LSN store for crash recovery
- **Allowlists** — Per-rule value and pattern allowlists to reduce false positives
- **Value masking** — Matched values are masked in alert output

## Quick Start

### Prerequisites

- Rust 2024 edition
- PostgreSQL 16+ with logical replication enabled (`wal_level = logical`)
- [just](https://github.com/casey/just) task runner

### 1. Enable logical replication

In `postgresql.conf`:

```
wal_level = logical
```

Create a publication for the tables you want to monitor:

```sql
CREATE PUBLICATION pgsense_pub FOR ALL TABLES;
```

### 2. Configure

```bash
cp config/app.toml my-config.toml
# edit my-config.toml — set postgres connection, choose alert channels
```

See [`config/app.toml`](config/app.toml) for all options with inline documentation.

### 3. Add detection rules

Edit or create a rules file. See [`config/rules.toml`](config/rules.toml) for the full reference with examples of each rule type (regex, builtin, script, allowlists).

### 4. Build and run

```bash
just build
just run scan -c my-config.toml -r config/rules.toml
```

## Detection Rules

Rules are defined in a separate TOML file. Three types are supported:

```toml
# Regex — pattern matching with optional validator
[[rules]]
id = "email-address"
pattern = '[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}'
category = "PII"
severity = "high"

# Builtin — algorithmic detection (no regex needed)
[[rules]]
type = "builtin"
id = "credit-card"
builtin = "credit_card"
category = "PCI_DSS"
severity = "critical"

# Script — custom Rhai logic
[[rules]]
type = "script"
id = "custom-detector"
script = "scripts/my_detector.rhai"
category = "CUSTOM"
severity = "medium"
```

Rules support `validate` (Luhn/SSN checksum), allowlists (exact values + patterns), and all standard fields. See [`config/rules.toml`](config/rules.toml) for the complete reference.

## CLI

```bash
pgsense-rs scan -c config/app.toml -r config/rules.toml               # start scanning
pgsense-rs scan -c config/app.toml -r config/rules.toml -vvv          # verbose
pgsense-rs rules list -r config/rules.toml                            # list loaded rules
pgsense-rs rules test -r config/rules.toml --input "4111111111111111" # test a value
```

## Monitoring

When `server.enabled = true`, an HTTP server exposes:

| Endpoint   | Description                          |
|------------|--------------------------------------|
| `/health`  | Always returns 200                   |
| `/ready`   | Returns 200 once the pipeline is up  |
| `/metrics` | Prometheus-format metrics            |

**Exported metrics**: `pgsense_events_total`, `pgsense_findings_total` (category, severity), `pgsense_alerts_total` (channel, status), `pgsense_scan_duration_seconds`.

## Development

### Prerequisites

- [just](https://github.com/casey/just) — task runner
- [cargo-nextest](https://nexte.st/) — test runner
- Docker (for integration tests)

### Nix Development Shell

```bash
nix develop # or: direnv allow
```

### Common Tasks

```bash
just build # Debug build (PROFILE=release for release)
just test  # Run all tests (nextest)
just lint  # Clippy + rustfmt check
just fmt   # Format code
just bench # Criterion benchmarks
just run scan -c config/app.toml -r config/rules.toml
```

### Project Structure

```
src/
├── main.rs              # Async entry point
├── lib.rs               # Module re-exports
├── args.rs              # CLI argument parsing (clap)
├── config.rs            # TOML config loading + env overrides
├── logging.rs           # Tracing setup
├── pipeline/            # etl integration, PipelineRunner
├── scanner.rs           # Event scanning, column-type filtering
├── rules/
│   ├── engine.rs        # RuleEngine (RegexSet fast-path)
│   ├── config.rs        # Rule/severity/validator types
│   ├── validators.rs    # Luhn, SSN validators
│   ├── builtin_detectors.rs  # Algorithmic CC/SSN detection
│   ├── masking.rs       # Value masking for output
│   └── script.rs        # Rhai script execution
├── alerts/
│   ├── dispatcher.rs    # Dedup + fan-out to channels
│   ├── dedup.rs         # Deduplication logic
│   ├── log.rs           # Structured log channel
│   ├── stdout.rs        # Stdout channel
│   ├── jsonl.rs         # JSONL file channel
│   ├── webhook.rs       # HTTP webhook channel
│   ├── slack.rs         # Slack channel (batched, with background flush)
│   └── postgres.rs      # PostgreSQL table channel
├── commands/
│   ├── scan.rs          # `scan` subcommand
│   └── rules.rs         # `rules list` / `rules test`
├── events.rs            # Event types from pipeline
├── watcher.rs           # File watcher for rules hot-reload
├── metrics.rs           # Prometheus metrics
└── server.rs            # Axum HTTP server (/health, /ready, /metrics)
```

### Docker

```bash
# Multi-arch build via GoReleaser (linux/amd64, linux/arm64)
goreleaser release --snapshot --clean

# Image: ghcr.io/mrehbr/pgsense-rs
```

## CI/CD

GitHub Actions workflows:

- **Checks** — Runs on every PR: build, test, lint, format
- **Prepare Release** — Manual workflow to create version tags
- **Publish Release** — Automatic binary releases and Docker images on tags

## License

MIT — see [LICENSE](LICENSE) for details.
