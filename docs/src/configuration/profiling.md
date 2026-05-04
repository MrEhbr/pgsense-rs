# Profiling

Profiling exposes two extra Prometheus histograms that record per-rule
and per-phase scan latency. They are gated behind a config flag because
they add cardinality (`rule_id` is unbounded by user config) and
measurable overhead in the hot scan loop.

> [!NOTE]
> Profiling is **off by default** and is intended to be enabled
> temporarily — long enough to gather a snapshot of rule-level latency,
> then turned back off.

## Configuration

```toml
[profiling]
enabled = false   # default
```

When `enabled = true`, two additional metrics are registered and
populated on every scanned value:

| Metric | Labels | Description |
|--------|--------|-------------|
| `pgsense_rule_scan_duration_seconds` | `rule_id` | Time spent evaluating a single rule against a value. |
| `pgsense_phase_scan_duration_seconds` | `phase` (one of `regex`, `builtin`, `script`) | Time spent in a scan phase. |

Both are histograms with sub-millisecond buckets. See
[Metrics](../ops/metrics.md) for the full list of exposed metrics.

When profiling is off, the scan loop incurs no measurement overhead.

## When to use it

Enable profiling when you need to answer questions like:

- *Which rule is dominating scan time?* — Sort
  `pgsense_rule_scan_duration_seconds_sum / _count` by `rule_id`.
- *Where is the rule engine spending its time across phases?* — Compare
  `pgsense_phase_scan_duration_seconds` for `regex` vs `builtin` vs
  `script`.

> [!CAUTION]
> `rule_id` is high-cardinality from Prometheus' perspective — every
> rule produces its own time series. With hundreds of rules, the
> resulting series count grows linearly. Disable profiling once you've
> captured what you need.
