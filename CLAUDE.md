# pgsense-rs

Rule-based PostgreSQL replication stream scanner for real-time sensitive data detection.

## Commands

```bash
just build              # Build (debug). PROFILE=release for release
just test               # Run all tests (nextest). Includes Docker integration tests
just lint               # Clippy (auto-fix) + rustfmt check
just fmt                # Format code
just bench              # Run criterion benchmarks
just run scan -c config/app.toml  # Run the scanner
```

## Architecture

**lib+bin crate** — `src/lib.rs` re-exports modules, `src/main.rs` is a thin async entry point.

```
Pipeline (supabase/etl) → ScannerDestination → mpsc → Scanner → Dispatcher → AlertChannels
```

Key modules:
- `pipeline/` — etl integration, `PipelineRunner` with `MemoryStore`/`PostgresStore` backends
- `rules/` — `RuleEngine` (RegexSet fast path), validators (Luhn, SSN), builtin rules, masking
- `scanner.rs` — scans events against rules, skips non-text column types
- `alerts/` — enum dispatch (`Log`/`Stdout`/`Webhook`), deduplication, dispatcher
- `commands/` — CLI: `rules`, `scan`
- `metrics.rs` / `server.rs` — Prometheus metrics, axum health endpoints

## Conventions

- `#![forbid(unsafe_code)]` in both lib.rs and main.rs
- Rust edition 2024, rustfmt max_width=160, imports_granularity=Crate
- Structs with `Default` impl use `#[serde(default)]` at struct level, not per-field functions
- `anyhow::Result` for error handling throughout
- Config: TOML-based with env override (`APP__*`). Example in `config/app.toml`
- etl dependency pinned to git rev `4f1141e` — requires PostgreSQL 16+

## Testing

- Unit tests inline (`#[cfg(test)]` modules)
- Integration tests in `tests/` — CLI tests via `assert_cmd`, pipeline tests via `testcontainers`
- Pipeline tests need Docker (Colima: `DOCKER_HOST=unix:///Users/ehbr/.colima/default/docker.sock`)
- Benchmarks in `benches/detection_bench.rs` (criterion)

## Dependencies to Know

- `supabase/etl` — replication streaming. Source at `~/.cargo/git/checkouts/etl-e302a20fce78b38f/4f1141e/`
- `etl::types::TableId`, `etl::store::both::{memory,postgres}`, `etl::pipeline::Pipeline`
- Rust `regex` crate has no lookahead/lookbehind — use simple patterns + validator functions
