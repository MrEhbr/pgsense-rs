# pgsense-rs

Rule-based PostgreSQL replication stream scanner for real-time sensitive data detection.

## Commands

```bash
just build              # Build (debug). PROFILE=release for release
just test               # Run all tests (nextest). Includes Docker integration tests
just lint               # Clippy (auto-fix) + rustfmt check
just fmt                # Format code
just bench              # Run criterion benchmarks
just run scan -c config/config.toml  # Run the scanner
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
- `pipeline/` — etl integration, `PipelineRunner` (one per database) with `MemoryStore`/`PostgresStore`/`SqliteStore` backends; `DatabaseConfig` for per-database connection + optional scan filter
- `pipeline/supervisor/` — `Supervisor` + `DatabaseUnit` lifecycle management (start, reconnect, shutdown per database)
- `rules/` — `RuleEngine` (RegexSet fast path), validators (Luhn, SSN), builtin rules, masking
- `events.rs` — `ScanEvent` extraction from etl events, `is_scannable_type()` column-type filtering
- `scanner.rs` — `Scanner::scan(event)` runs rules against scan events; skips non-text column types
- `watcher.rs` — file watcher for hot-reloading rules via `notify`
- `alerts/` — enum dispatch (`Log`/`Stdout`/`Jsonl`/`Webhook`/`Slack`/`Postgres`), deduplication, dispatcher
- `commands/` — CLI: `rules`, `scan`
- `metrics.rs` / `server.rs` — Prometheus metrics (14 counters/gauges/histograms), axum health endpoints

## Conventions

- `#![forbid(unsafe_code)]` in both lib.rs and main.rs
- Rust edition 2024, rustfmt max_width=160, imports_granularity=Crate
- Structs with `Default` impl use `#[serde(default)]` at struct level, not per-field functions
- `anyhow::Result` for error handling throughout
- Config: TOML-based with env override (`APP__*`). Example in `config/config.toml`
- etl dependency pinned to git rev `4f1141e` — requires PostgreSQL 16+

## Testing

- Unit tests inline (`#[cfg(test)]` modules)
- Integration tests in `tests/` — CLI tests via `assert_cmd`, pipeline tests via `testcontainers`
- Pipeline tests need Docker (Colima: `DOCKER_HOST=unix:///Users/ehbr/.colima/default/docker.sock`)
- Benchmarks in `benches/` (criterion): `detection_bench.rs`, `builtin_detectors_bench.rs`, `validators_bench.rs`

## Dependencies to Know

- `supabase/etl` — replication streaming. Source at `~/.cargo/git/checkouts/etl-e302a20fce78b38f/4f1141e/`
- `etl::types::TableId`, `etl::store::both::memory::MemoryStore`, `etl::pipeline::Pipeline`
- Rust `regex` crate has no lookahead/lookbehind — use simple patterns + validator functions
