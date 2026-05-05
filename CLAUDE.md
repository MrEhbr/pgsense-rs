# pgsense-rs

Rule-based PostgreSQL replication stream scanner for real-time sensitive data detection.

## Commands

```bash
just build              # Build (debug). PROFILE=release for release
just test               # Run all tests (nextest). Includes Docker integration tests
just lint               # Clippy (auto-fix) + rustfmt check
just fmt                # Format code
just bench              # Run criterion benchmarks
just typos              # Run typo checker (writes fixes)
just run scan -c config/config.toml      # Run the scanner
just run validate -c config/config.toml  # Validate config (add --connect to test DB connectivity)
just run rules list                      # List active detection rules
just fuzz <target>      # Run AFL fuzz target (credit_cards, ssns, phones, emails, ibans)
just dev                # Start dev environment (PostgreSQL only; add --profile bench for Grafana/Prometheus)
just dev-stop           # Stop dev environment
just dev-clean          # Remove dev environment (volumes + orphans)
just test-coverage      # Generate code coverage report (requires cargo-llvm-cov)
```

## Architecture

**lib+bin crate** — `src/lib.rs` re-exports modules, `src/main.rs` is a thin async entry point.

```
Supervisor
├─ DatabaseUnit("host1/db") → PipelineRunner → ScannerDestination → mpsc → scan_loop
├─ DatabaseUnit("host2/db") → PipelineRunner → ScannerDestination → mpsc → scan_loop
                                                                              │
                                                                    Scanner::scan(event)
                                                                              │
                                                                    Dispatcher → AlertChannels
```

Key modules:
- `pipeline/` — etl integration, `PipelineRunner` (one per database) using etl's own `MemoryStore` or `PostgresStore` (state co-located in source DB under `etl` schema); `DatabaseConfig` for per-database connection + optional scan filter; `source_bootstrap.rs` installs `etl.describe_table_schema()` helpers required by etl ≥ `ce88ba7`
- `pipeline/supervisor/` — `Supervisor` + `DatabaseUnit` lifecycle management (start, reconnect, shutdown per database)
- `rules/` — `RuleEngine` three-phase scan (regex+validators → builtin detectors → Rhai scripts), validators (Luhn, SSN, phone, email, IBAN), masking. `RuleConfig.rule_type` (serde `"type"`) selects Regex/Builtin/Script
- `events.rs` — `ScanEvent` extraction from etl events, `is_scannable_type()` column-type filtering
- `scanner.rs` — `Scanner::scan(event)` runs rules against scan events
- `watcher.rs` — file watcher for hot-reloading rules via `notify`
- `alerts/` — enum dispatch (`Log`/`Stdout`/`Jsonl`/`Webhook`/`Slack`/`Postgres`), deduplication, dispatcher
- `commands/` — CLI: `rules`, `scan`, `validate` (with `validate connect` subcommand to test DB connectivity)
- `config.rs` — top-level `Config`, TOML/env loading, password file resolution
- `args.rs` — CLI argument parsing + `route()` dispatch
- `logging.rs` — tracing subscriber setup, file logging, JSON/text format
- `pattern.rs` — glob pattern matching for scan filter includes/excludes
- `rules/detectors/` — builtin detectors: credit card, SSN, phone, email, IBAN
- `metrics.rs` / `server.rs` — Prometheus metrics (counters/gauges/histograms), axum health endpoints
- `telemetry.rs` — OpenTelemetry tracer setup (gated by `otel` Cargo feature)
- `validation.rs` — `Validate` trait for config-time error reporting

## Conventions

- `#![forbid(unsafe_code)]` in both lib.rs and main.rs
- Rust edition 2024, rustfmt max_width=160, imports_granularity=Crate
- Structs with `Default` impl use `#[serde(default)]` at struct level, not per-field functions
- `anyhow::Result` for error handling throughout
- Config: TOML-based with env override (`PGSENSE__*`). Examples in `config/config.toml` (settings) and `config/rules.toml` (rules; selected by `rules_file` config key)
- Cargo features: default is none. Optional `tokio-console` (enables `console-subscriber`) and `otel` (enables OpenTelemetry OTLP export via `tonic` and `opentelemetry*` crates)
- etl dependency pinned to git rev `ce88ba7` — requires PostgreSQL 16+
- `docs/` — mdBook documentation site, served via `just docs`, deployed to GitHub Pages
- `charts/pgsense/` — Helm chart for Kubernetes deployment

## Testing

- Unit tests inline (`#[cfg(test)]` modules)
- Integration tests in `tests/` — CLI tests via `assert_cmd`, pipeline tests via `testcontainers`
- Pipeline tests need Docker. On macOS with Colima: `export DOCKER_HOST=unix://$HOME/.colima/default/docker.sock`
- Benchmarks in `benches/` (criterion): `detection_bench.rs`, `builtin_detectors_bench.rs`, `validators_bench.rs`
- Fuzz targets in `fuzz/` (AFL): credit_cards, ssns, phones, emails, ibans

## Dependencies to Know

- `supabase/etl` — replication streaming. Source at `~/.cargo/git/checkouts/etl-e302a20fce78b38f/ce88ba7/`
- `etl::types::TableId`, `etl::store::{MemoryStore, PostgresStore}`, `etl::pipeline::Pipeline`
- Rust `regex` crate has no lookahead/lookbehind — use simple patterns + validator functions
