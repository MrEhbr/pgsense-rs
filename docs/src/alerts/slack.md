# Slack Channel

Sends formatted Slack messages with batching and rate-limit handling.
Multiple Slack channels can be configured — each is an independent
`[[alerts.slack]]` block.

## Configuration

```toml
[[alerts.slack]]
name            = "security-slack"      # optional — defaults to "slack" or "slack-N"
token           = "xoxb-your-bot-token" # required — inline value, or:
# token         = { file = "/run/secrets/slack-token" }   # read from a file
channel         = "#pgsense-alerts"     # required — channel name or ID
username        = "pgsense-bot"         # optional display name
icon_emoji      = ":shield:"            # optional emoji
timeout_ms      = 5000                  # default 5000
batch_size      = 8                     # findings per message; default 8
batch_window_ms = 2000                  # max wait before flushing a partial batch; default 2000
max_retries     = 3                     # retry attempts on 429 responses; default 3
```

> [!IMPORTANT]
> The `token` field is a secret. For production deployments, prefer the
> file-backed form (`token = { file = "..." }`) so the value never lives
> in plaintext config or process environment. The bot needs `chat:write` (and
> `chat:write.public` if posting to channels the bot isn't a member of).

## Batching

Findings are buffered for up to `batch_window_ms` (default 2 s) or until
`batch_size` is reached, whichever happens first. Each batch becomes one
Slack message containing one block per finding.

> [!NOTE]
> Slack imposes a 50-block limit per message and the scanner uses ~3
> blocks per finding (header + section + context). Keep `batch_size`
> ≤ 8 to stay well below Slack's hard limits.

## Single-finding format

When a batch contains exactly one finding, the message uses a richer
single-finding layout: a section with rule + category + database +
table + column fields, then the masked sample. Multi-finding batches
use a more compact header + per-finding block layout.

Both layouts color-code by severity (red for `critical` through grey
for `info`) and prefix with a matching emoji.

## Rate limiting

When Slack returns `429 Too Many Requests`, the channel respects the
`Retry-After` header and retries up to `max_retries` times with
exponential back-off on top. Findings that exhaust retries are counted
in `pgsense_alerts_total{channel,status="error"}` and dropped.

## Multiple Slack destinations

```toml
[[alerts.slack]]
name    = "security-slack"
token   = "xoxb-..."
channel = "#sec-alerts"

[[alerts.slack]]
name    = "ops-slack"
token   = "xoxb-..."
channel = "#oncall"
```

Combine with per-rule `channels = [...]` to route critical findings to
security and lower-severity findings to ops.
