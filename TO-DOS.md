# TO-DOS

## NATS Alert Channel - 2026-02-12 23:31

- **Add NATS alert channel** - Publish findings as JSON to a NATS subject. Lightweight alternative to Kafka for event distribution. **Problem:** No lightweight pub/sub integration — Kafka is heavyweight for smaller deployments; NATS provides a simpler alternative with optional durability via JetStream. **Files:** `src/alerts/mod.rs:18-49`, `src/alerts/config.rs:1-73`, `src/alerts/dispatcher.rs`. **Solution:** Add `NatsChannel` variant to `AlertChannel` enum, `NatsConfig` to `AlertsConfig` (url, subject, optional credentials/TLS, JetStream toggle for at-least-once delivery). Use `async-nats` crate. Gate behind `nats` feature flag.
