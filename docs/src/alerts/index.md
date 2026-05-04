# Alert Channels

When the rule engine produces a finding, the alert dispatcher fans it out to
every configured channel — or to the subset listed on the rule's `channels`
field. Channels are configured under `[alerts.<name>]` (or
`[[alerts.<name>s]]` for repeatable channels) in `config.toml`.

## Available channels

`Log`
:   Structured event emitted through the same logger as everything else
    in the process.

`Stdout`
:   Plain-text line on stdout, bypassing the logger.

`JSONL`
:   Append a JSON object per finding to a file.

`Webhook` (repeatable)
:   POST a JSON payload to an HTTPS endpoint with custom headers.

`Slack` (repeatable)
:   Formatted Slack message with batching and rate-limit handling.

`PostgreSQL`
:   Insert findings into a target Postgres table (auto-created).

## Enabling channels

```toml
[alerts]
dedup_window_seconds = 300       # default 300 (5 min); 0 disables dedup

[alerts.log]
enabled = true                   # default true

[alerts.stdout]
enabled = true                   # default false

[alerts.jsonl]
enabled = true                   # default false
path    = "/var/log/pgsense/findings.jsonl"

# Webhook is repeatable — add multiple [[alerts.webhooks]] blocks
[[alerts.webhooks]]
name       = "security-webhook"
url        = "https://example.com/hooks/pgsense"
timeout_ms = 5000
[alerts.webhooks.headers]
Authorization = "Bearer ..."
```

> [!NOTE]
> The default config enables only the `log` channel. Add a
> `[alerts.<name>]` block (or `[[alerts.webhooks]]` / `[[alerts.slack]]`)
> to bring up additional channels.

## Channel names

Each channel has a name used for per-rule routing and as the `channel`
label on `pgsense_alerts_total`:

`log`, `stdout`, `postgres`
:   Fixed names — only one of each can exist.

`jsonl`
:   Defaults to `"jsonl"`, override with `name = "..."` on the config.

`webhook` / `webhook-N`
:   Single webhook gets `"webhook"`. Multiple webhooks get `"webhook-1"`,
    `"webhook-2"`, etc. Override with `name = "..."` on the entry.

`slack` / `slack-N`
:   Same pattern as webhook.

## Per-rule routing

Add a `channels` array to a rule to restrict where its findings go:

```toml
[[rules]]
id          = "credit-card"
description = "Credit card numbers"
type        = "builtin"
builtin     = "credit_card"
category    = "PCI_DSS"
severity    = "critical"
channels    = ["security-slack", "postgres"]   # log/stdout/jsonl/webhooks are skipped
```

Channel names in the array match the resolved name (custom `name` field
or the default like `webhook-1`). When `channels` is omitted, the
finding goes to every enabled channel.

> [!TIP]
> The `validate` CLI subcommand emits a warning when a rule references a
> channel name that no enabled channel matches. Run it after editing
> rules or alert config to catch typos before they cause silently-dropped
> findings.

## Deduplication

Before fan-out, the dispatcher checks a deduplication cache keyed by
`(database, schema, table, column, rule_id, hash(value))`. Repeated
findings inside the configured window are suppressed and counted in
`pgsense_dedup_total`. See [Deduplication](./deduplication.md) for the
gory details.

## Where to go next

Each channel has its own page in this part of the manual with full
configuration options, payload format, and operational caveats.
