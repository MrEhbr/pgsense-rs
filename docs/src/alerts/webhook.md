# Webhook Channel

POSTs each finding as JSON to an HTTPS endpoint. Multiple webhooks can
be configured — each is an independent `[[alerts.webhooks]]` block.

## Configuration

```toml
[[alerts.webhooks]]
name       = "security-webhook"   # optional — defaults to "webhook" or "webhook-N"
url        = "https://hooks.example.com/alert"   # required
timeout_ms = 5000                 # default 5000

[alerts.webhooks.headers]
Authorization = "Bearer token123"
X-Source      = "pgsense-rs"
# Or load any header value from a file:
# Authorization = { file = "/run/secrets/webhook-auth" }
```

> [!IMPORTANT]
> Header values are treated as **secrets** — they are skipped in any
> serialization round-trip. For production deployments, prefer the
> file-backed form (`Authorization = { file = "..." }`) so values never
> live in plaintext config or process environment.

## Multiple webhooks

```toml
[[alerts.webhooks]]
name = "siem"
url  = "https://siem.example.com/ingest"

[[alerts.webhooks]]
name = "incident-bot"
url  = "https://hooks.example.com/incidents"
```

Both receive every finding (subject to per-rule routing).

## Per-rule routing

Use the `channels` field on a rule to send findings to a specific
webhook by `name`:

```toml
[[rules]]
type        = "builtin"
id          = "credit-card"
description = "Credit card numbers"
category    = "PCI_DSS"
severity    = "critical"
builtin     = "credit_card"
channels    = ["siem"]   # only the "siem" webhook gets this rule's findings
```

## Payload

The body is a JSON object with the same fields as the
[JSONL Channel](./jsonl.md). The `masked_sample` is masked — see
[Masking](../rules/masking.md).

## Error handling

> [!WARNING]
> A non-2xx response or a network error is logged and counted in
> `pgsense_alerts_total{channel="<name>",status="error"}`. The scanner
> does **not retry** — webhook delivery is best-effort. For
> at-least-once delivery, point the webhook at a queue (Kafka HTTP
> proxy, SQS HTTP forwarder, etc.) and let that subsystem handle
> retries and durability.

The URL is validated at startup (must start with `http://` or
`https://`).
