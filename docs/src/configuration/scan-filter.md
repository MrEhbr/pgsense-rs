# Scan Filter

The scan filter narrows what pgsense-rs sends to the rule engine. It works at
two levels — the top-level `[scan]` block applies to every database that
doesn't override it; a per-database `[databases.scan]` block fully replaces
the top-level filter for that database.

## Fields

All fields support exact strings and glob patterns (`*` and `?` wildcards).

```toml
[scan]
include_schemas = []   # only scan these schemas (empty = all). e.g. ["staging_*"]
exclude_tables  = []   # skip these tables. e.g. ["audit_*", "tmp_*"]
exclude_columns = []   # skip these columns. e.g. ["*_hash", "*_token"]
```

`include_schemas`
:   If non-empty, only events from listed schemas are scanned.

`exclude_tables`
:   Tables matching any pattern are skipped, even if their schema is included.

`exclude_columns`
:   Columns matching any pattern are skipped at the value level.

## Per-database override

```toml
[scan]
include_schemas = ["public"]

[[databases]]
host = "shared.example.com"
dbname = "app"
username = "pgsense"
password = { file = "/run/secrets/app-pw" }

[[databases]]
host = "shared.example.com"
dbname = "analytics"
username = "pgsense"
password = { file = "/run/secrets/analytics-pw" }

# Analytics has its own filter — top-level [scan] does NOT merge in.
[databases.scan]
include_schemas = ["events_*"]
exclude_columns = ["*_hash", "raw_payload"]
```

> [!NOTE]
> A per-database `[databases.scan]` block fully replaces the top-level
> `[scan]` for that database — fields are not merged. If you want a
> common base filter, repeat the relevant entries inside each
> `[databases.scan]` block.

## Column-type filtering (always on)

In addition to user-configured filters, pgsense-rs automatically skips
columns whose Postgres type can't contain meaningful text — `bool`,
`int2`/`int4`/`int8`, `float4`/`float8`, `numeric`, `oid`, `bytea`,
`uuid`, `date`, `time`/`timetz`, `timestamp`/`timestamptz`, `interval`.
This happens before user filters and cannot be disabled.

## Per-rule scope

Filters here are global (per database). To restrict an individual rule, use
`[rules.scope]` on that rule — see [Allowlists & Scope](../rules/scope.md).
