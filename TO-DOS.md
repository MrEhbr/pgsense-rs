# TO-DOS

## PostgreSQL Alert Channel - 2026-02-12 23:31

- **Add PostgreSQL alert channel** - Write findings to a queryable PG table (`pgsense_findings`). Enables dashboarding, retention policies, and `SELECT`-based auditing. **Problem:** No persistent queryable storage for findings — current channels are transient (log, stdout) or append-only files (JSONL). **Files:** `src/alerts/mod.rs:18-49`, `src/alerts/config.rs:1-73`, `src/alerts/dispatcher.rs`, `src/pipeline/store/postgres/`. **Solution:** Add `PostgresChannel` variant to `AlertChannel` enum, `PostgresAlertConfig` to `AlertsConfig` (connection params or reuse store connection, schema, table name). Reuse existing `sqlx`/PG infra from the postgres store.

## NATS Alert Channel - 2026-02-12 23:31

- **Add NATS alert channel** - Publish findings as JSON to a NATS subject. Lightweight alternative to Kafka for event distribution. **Problem:** No lightweight pub/sub integration — Kafka is heavyweight for smaller deployments; NATS provides a simpler alternative with optional durability via JetStream. **Files:** `src/alerts/mod.rs:18-49`, `src/alerts/config.rs:1-73`, `src/alerts/dispatcher.rs`. **Solution:** Add `NatsChannel` variant to `AlertChannel` enum, `NatsConfig` to `AlertsConfig` (url, subject, optional credentials/TLS, JetStream toggle for at-least-once delivery). Use `async-nats` crate. Gate behind `nats` feature flag.
