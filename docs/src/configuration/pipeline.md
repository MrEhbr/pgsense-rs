# Pipeline Tuning

The `[pipeline]` block controls how the replication-stream consumer
batches events, retries failures, and provisions worker concurrency.
The `store` field is documented separately under
[State Store](./store.md); the rest of the block is covered here.

> [!NOTE]
> The defaults are sensible for passive scanning workloads — most
> deployments never need to touch these. Adjust only when metrics show
> a specific bottleneck.

## Fields

```toml
[pipeline]
store                          = "memory"   # see State Store
batch_max_fill_ms              = 1000       # default 1000 (1s)
batch_memory_budget_ratio      = 0.2        # default 0.2 (20% of process memory)
table_error_retry_delay_ms     = 10000      # default 10000 (10s)
table_error_retry_max_attempts = 5          # default 5
max_table_sync_workers         = 4          # default 4
max_copy_connections_per_table = 2          # default 2
memory_refresh_interval_ms     = 100        # default 100
```

### `batch_max_fill_ms`

Maximum time the pipeline holds a batch open before flushing it
downstream, in milliseconds. Lower values reduce end-to-end alert
latency at the cost of more frequent dispatch overhead. Default is
1 000 ms — tuned for detection latency rather than throughput. Raise
it only if you observe the scanner spending most of its time on
dispatch.

### `batch_memory_budget_ratio`

Fraction of the process's resident memory budget the pipeline may use
for in-flight batches before forcing a flush, expressed as `0.0..=1.0`.
The default of `0.2` (20%) leaves room for the rule engine, dedup
cache, and alert channel buffers.

### `table_error_retry_delay_ms` / `table_error_retry_max_attempts`

When a per-table operation fails (typically during initial table sync,
or on a transient query error), the pipeline pauses for
`table_error_retry_delay_ms` milliseconds before retrying, up to
`table_error_retry_max_attempts` times. After the cap is hit the table
is left in an error state and the failure is logged; other tables are
unaffected.

The defaults (10 s × 5 attempts ≈ 50 s of recovery window) are tuned
for the kind of transient errors a replication subscriber typically
sees (lock contention, brief connection blips). Raise the attempt count
in environments with longer maintenance windows.

### `max_table_sync_workers`

Maximum number of concurrent worker tasks performing initial table
syncs (the COPY phase that runs before streaming WAL changes for a
newly-published table). Default is 4. Workers run within the same
process and share the database connection limit; raising this number
beyond what the source can sustain will not improve throughput.

### `max_copy_connections_per_table`

Maximum number of parallel connections used for the initial COPY of a
single table. Default is 2. Most tables don't benefit from more than
2 — go higher only for very large tables on a server with spare
connection slots.

> [!CAUTION]
> Effective connection load on the source is roughly
> `max_table_sync_workers × max_copy_connections_per_table` plus one
> persistent replication slot connection per `[[databases]]` entry.
> Verify the result against the server's `max_connections`.

### `memory_refresh_interval_ms`

Interval, in milliseconds, at which the pipeline samples its own
memory usage to enforce `batch_memory_budget_ratio`. Default 100 ms.
Lower values give tighter back-pressure but cost CPU; the default is
appropriate for nearly all deployments.
