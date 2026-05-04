# Log Channel

Emits each finding as a structured event through the same logger
configured under `[log]` — same level, format, and output.

## Configuration

```toml
[alerts.log]
enabled = true   # default true
```

This channel is enabled by default. Set `enabled = false` to suppress
findings from the log stream (they will still go to other configured
channels).

## Severity → log level

Each finding's severity maps to a log level so you can filter findings
the same way you filter any other application log:

| Severity | Log level |
|----------|-----------|
| `critical` | `error` |
| `high` | `error` |
| `medium` | `warn` |
| `low` | `warn` |
| `info` | `info` |

> [!IMPORTANT]
> `critical` and `high` findings emit at `error` level. If your
> `[log] level` is set to `warn` or above, they still appear; setting
> the level to `error` will hide medium/low/info findings from the
> log stream entirely.

## Event fields

Every emitted event carries these structured fields:

- `database`
- `rule_id`
- `category`
- `severity`
- `schema`
- `table`
- `column`
- `sample` (the masked match — see [Masking](../rules/masking.md))
- `primary_key` (formatted as `k1=v1,k2=v2`)
- `lsn`

The static message is `"sensitive data detected"`.

## Use cases

- Local development — findings appear in `journalctl` / container logs.
- Centralized logging — set `[log] format = "json"` and ship stderr to
  Loki, ELK, Cloudwatch, etc.
- Debugging rule false positives without setting up another channel.
