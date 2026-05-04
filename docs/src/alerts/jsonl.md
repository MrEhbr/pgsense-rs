# JSONL Channel

Appends one JSON object per finding to a file. Stable format suitable
for offline analysis or as a feed into a log shipper.

## Configuration

```toml
[alerts.jsonl]
enabled = false           # default false
name    = "jsonl"         # optional — override default channel name
path    = "alerts.jsonl"  # output file path (default "alerts.jsonl", relative to CWD)
```

The file is opened in append mode at startup. Parent directories are
created automatically if missing.

## Output

Each line is one finding as a JSON object — same shape as the webhook
payload:

```json
{
  "database": "primary/orders",
  "rule_id": "email-address",
  "description": "Email addresses",
  "category": "PII",
  "severity": "HIGH",
  "schema_name": "public",
  "table_name": "users",
  "column_name": "contact",
  "masked_sample": "ja****om",
  "primary_keys": [["id", "42"]],
  "lsn": 1234567
}
```

The `masked_sample` field is masked — see [Masking](../rules/masking.md).
`primary_keys` is an array of `[name, value]` pairs.

## Operational notes

> [!WARNING]
> The file is opened once at scanner startup and held open. If you
> rotate the file out from under the process (e.g. `mv alerts.jsonl
> alerts.old && touch alerts.jsonl`), pgsense-rs continues writing to the
> moved inode until restart. For log rotation, prefer the
> [Log Channel](./log.md) with a real log shipper, or use a webhook.

Each finding is written through a `BufWriter` and the writer is flushed
immediately after every line, so no findings stay buffered between
writes.
