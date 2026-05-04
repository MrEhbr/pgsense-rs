# pgsense-rs

[![CI](https://github.com/MrEhbr/pgsense-rs/actions/workflows/checks.yml/badge.svg)](https://github.com/MrEhbr/pgsense-rs/actions)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024+-orange)](https://www.rust-lang.org)

A rule-based service that monitors PostgreSQL logical replication streams to detect sensitive data and trigger real-time alerts.

## Why pgsense-rs?

Sensitive data — credit card numbers, SSNs, email addresses, API keys — ends up in production databases through application bugs, unvalidated imports, or missing input filters. By the time you notice, the data is already persisted and potentially exposed.

pgsense-rs watches the PostgreSQL WAL in real time, catching sensitive values **as they're written** rather than after the fact. It runs as a separate process with zero changes to your application code or schema.

## How It Works

```
PostgreSQL WAL → etl Pipeline → Scanner → Rule Engine → Alert Dispatcher
```

Three-phase rule engine: regex (with optional `luhn` / `ssn` / `phone` / `email` / `iban` validators), built-in algorithmic detectors (credit cards, IBANs, SSNs, phone numbers, emails), and Rhai scripts for custom logic. Findings are deduplicated and routed to one or more alert channels — log, stdout, JSONL, webhook, Slack, or PostgreSQL.

## Features

- Real-time scanning of INSERT/UPDATE events
- Three rule types — regex + validators, built-in detectors, Rhai scripts
- Hot reload for the rules file (no scanner restart)
- Multi-database — monitor several PostgreSQL databases concurrently from one process
- Per-rule scope (schemas/tables/columns) and allowlists for false-positive control
- Value masking before alerting — raw values never leave the scanner
- Multiple alert channels with per-rule routing
- Persistent checkpointing — memory (default) or PostgreSQL-backed LSN store
- Prometheus metrics, health endpoints, Helm chart

## Quick Start

```bash
# 1. Enable logical replication on your PostgreSQL server
#    (postgresql.conf: wal_level = logical, then restart)
psql -c "CREATE PUBLICATION pgsense_pub FOR ALL TABLES;"

# 2. Configure
cp config/config.toml my-config.toml
# edit my-config.toml — set databases + alert channels

# 3. Validate before running
pgsense-rs validate -c my-config.toml --connect

# 4. Run
pgsense-rs scan -c my-config.toml -r config/rules.toml
```

The bundled [`config/config.toml`](config/config.toml) and [`config/rules.toml`](config/rules.toml) have inline documentation for every option.

## Documentation

Full documentation lives in [`docs/`](docs/) and is published to
**[https://mrehbr.github.io/pgsense-rs/](https://mrehbr.github.io/pgsense-rs/)**.
Highlights:

- **[Quick Start](https://mrehbr.github.io/pgsense-rs/getting-started/quick-start.html)** — first scan in five minutes
- **[PostgreSQL Setup](https://mrehbr.github.io/pgsense-rs/getting-started/postgres-setup.html)** — server prerequisites & cloud-provider notes
- **[Configuration Reference](https://mrehbr.github.io/pgsense-rs/configuration/)** — full TOML schema
- **[Detection Rules](https://mrehbr.github.io/pgsense-rs/rules/)** — regex, builtin, Rhai scripts, allowlists, scope
- **[Alert Channels](https://mrehbr.github.io/pgsense-rs/alerts/)** — log, stdout, JSONL, webhook, Slack, PostgreSQL
- **[CLI Reference](https://mrehbr.github.io/pgsense-rs/cli/scan.html)** — `scan`, `rules`, `validate`
- **[Operations](https://mrehbr.github.io/pgsense-rs/ops/metrics.html)** — metrics, health, Helm, multi-database

Build the docs locally with `just docs-serve`.

## Development

### Prerequisites

- [just](https://github.com/casey/just) — task runner
- [cargo-nextest](https://nexte.st/) — test runner
- Docker (for integration tests)
- [mdBook](https://rust-lang.github.io/mdBook/) — for docs (`just docs-serve`)

### Nix Development Shell

```bash
nix develop   # or: direnv allow
```

The dev shell pins the toolchain and includes every tool listed above.

### Common Tasks

```bash
just build         # Debug build (PROFILE=release for release)
just test          # Run all tests (nextest)
just lint          # Clippy + rustfmt check
just fmt           # Format code
just bench         # Criterion benchmarks
just docs-serve    # Live-preview the documentation site
```

### Docker / Releases

Multi-arch images (`linux/amd64`, `linux/arm64`) are published to
`ghcr.io/mrehbr/pgsense-rs` on every release tag. Local snapshot builds:

```bash
goreleaser release --snapshot --clean
```

## CI/CD

- **Checks** — build, test, lint, format on every PR
- **Docs** — `mdbook` build on PRs touching `docs/`; deploy to GitHub Pages on `main`
- **Prepare Release** — manual workflow to create version tags
- **Publish Release** — binary releases and Docker images on tags

## License

MIT — see [LICENSE](LICENSE) for details.
