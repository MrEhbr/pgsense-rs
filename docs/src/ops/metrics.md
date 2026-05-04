# Metrics

When the [server](../configuration/server.md) is enabled, Prometheus
metrics are served at `GET /metrics` in the standard text format
(`text/plain; version=0.0.4`).

## Exported metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `pgsense_events_total` | counter | `database` | Replication events processed. |
| `pgsense_findings_total` | counter | `database`, `category`, `severity` | Sensitive data findings produced. |
| `pgsense_alerts_total` | counter | `channel`, `status` | Alerts dispatched (`status` is `ok` or `error`). |
| `pgsense_pipeline_reconnects_total` | counter | `database` | Pipeline reconnection attempts. |
| `pgsense_events_skipped_total` | counter | `database`, `reason` | Events skipped by scan filters. |
| `pgsense_dedup_total` | counter | `database`, `outcome` | Dedup decisions (`suppressed` or `passed`). |
| `pgsense_config_reloads_total` | counter | `status` | Rule-file reload attempts (`ok` or `error`). |
| `pgsense_script_errors_total` | counter | `rule_id` | Rhai script execution errors. |
| `pgsense_rules_loaded` | gauge | — | Detection rules currently loaded. |
| `pgsense_pipeline_connected` | gauge | `database` | Pipeline connection state (`1` connected, `0` disconnected). |
| `pgsense_queue_depth` | gauge | `database` | Pending batches in the event channel. |
| `pgsense_scan_duration_seconds` | histogram | `database` | Time spent scanning a single event. |
| `pgsense_batch_size` | histogram | `database` | Events per pipeline batch. |
| `pgsense_dispatch_duration_seconds` | histogram | `database` | Time spent dispatching alerts for one event. |
| `pgsense_rule_scan_duration_seconds` | histogram | `rule_id` | Per-rule scan latency. **Profiling only.** |
| `pgsense_phase_scan_duration_seconds` | histogram | `phase` | Per-phase scan latency (`regex`, `builtin`, `script`). **Profiling only.** |
| `process_*` | mixed | — | CPU, memory, open FDs, start time. **Linux only.** |

## Profiling

Per-rule and per-phase histograms are gated behind the
`profiling.enabled` flag because they significantly increase metric
cardinality:

```toml
[profiling]
enabled = true
```

> [!TIP]
> Enable profiling temporarily when investigating performance issues
> (some rule is slowing the scanner down) and disable it in
> steady-state production. The combined cardinality of
> `pgsense_rule_scan_duration_seconds{rule_id}` for hundreds of rules
> across many time-series buckets adds up quickly.

## Suggested alerts

| Condition | Suggests |
|-----------|----------|
| `rate(pgsense_pipeline_reconnects_total[5m]) > 0` | Replication slot or network instability. |
| `pgsense_pipeline_connected == 0` for > 1 min | Pipeline is down — scanner is producing zero findings for that database. |
| `rate(pgsense_alerts_total{status="error"}[5m]) > 0` | Some alert channel is failing. |
| `rate(pgsense_config_reloads_total{status="error"}[5m]) > 0` | A recent rules-file edit failed to compile and was rolled back. |
| `rate(pgsense_script_errors_total[5m]) > 0` | A Rhai script is failing or timing out. |
| `pgsense_queue_depth` rising over time | Scanner can't keep up with replication throughput. |

## Cardinality budget

The high-cardinality labels are `database` (one per `[[databases]]`
entry) and `rule_id` (only on the profiling histogram). Other labels
are bounded — typical deployments emit a few hundred series total even
with several databases configured.
