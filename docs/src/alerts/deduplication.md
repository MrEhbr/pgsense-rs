# Deduplication

Repeated findings for the same value in the same column are suppressed
within a configurable time window. This keeps alerts actionable when a
sensitive value is written, updated, or replicated repeatedly.

## Configuration

```toml
[alerts]
dedup_window_seconds = 300   # default 300 (5 minutes); 0 disables dedup
```

## Dedup key

The dispatcher keys the cache by the tuple:

```
(database, schema, table, column, rule_id, hash(value))
```

The first finding for a given key passes through to all configured
channels. Subsequent findings inside the window are dropped and counted
in `pgsense_dedup_total{database, outcome="suppressed"}`.

The hash is non-cryptographic and operates on the matched text. The raw
value never leaves the scanner — only its hash and the masked sample.

## Cache size

The cache is bounded at **10 000 entries**. When it grows beyond this
threshold, expired entries (older than the configured window) are
pruned in bulk on the next lookup. In normal operation this bound is
rarely hit.

## Disabling

Set `dedup_window_seconds = 0` to disable dedup entirely. Every finding
goes to every configured channel.

> [!CAUTION]
> Disabling dedup is rarely the right choice. Even a `REPLICA IDENTITY
> FULL` table that gets a column-unrelated UPDATE will fire a
> logical-replication event that re-scans the unchanged sensitive
> column, generating a duplicate finding. With dedup off, expect to see
> repeat alerts for every row mutation.

## Per-database scope

Dedup is **per-database**. The same value in the same column name across
two different databases (`primary/orders` vs `secondary/orders`) is
treated as two distinct findings — both pass dedup independently.

## Metrics

| Metric | Description |
|--------|-------------|
| `pgsense_dedup_total{database, outcome="suppressed"}` | Findings dropped by dedup. |
| `pgsense_dedup_total{database, outcome="passed"}` | Findings that passed dedup and reached channels. |
