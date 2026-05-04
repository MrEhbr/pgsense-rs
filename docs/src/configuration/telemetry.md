# Telemetry (OpenTelemetry)

When enabled, pgsense-rs exports tracing spans over OTLP to a collector
of your choice (Tempo, Jaeger, Honeycomb, an OTel collector
sidecar, etc.). Spans are produced from the `tracing` instrumentation
already present throughout the codebase — pipeline lifecycle, scan
loops, rule evaluation, alert dispatch.

> [!NOTE]
> Telemetry is **off by default**. The `[telemetry]` block only takes
> effect when `enabled = true`.

## Fields

```toml
[telemetry]
enabled      = false                     # default false
endpoint     = "http://localhost:4317"   # default
protocol     = "grpc"                    # "grpc" | "http"
service_name = "pgsense"                 # default
sample_rate  = 1.0                       # 0.0 – 1.0
```

### `enabled`

Master switch. When `false`, no exporter is started and no telemetry
overhead is incurred.

### `endpoint`

OTLP endpoint URL of your collector. The default targets a local
collector listening on the standard OTLP gRPC port (`4317`). For OTLP
HTTP, point at the collector's HTTP endpoint (typically `:4318`).

### `protocol`

Transport for OTLP. Two values are accepted:

`grpc` (default)
:   OTLP/gRPC — the lower-overhead transport, supported by every
    mainstream collector.

`http`
:   OTLP/HTTP (protobuf-over-HTTP). Use when gRPC is impractical (egress
    rules, sidecar HTTP-only collector, etc.).

### `service_name`

Value emitted as the OTel `service.name` resource attribute. Defaults
to `pgsense`. Override when running multiple pgsense-rs deployments
that share a collector and you want them to appear as distinct
services.

### `sample_rate`

Trace sampler ratio in `0.0..=1.0`:

- `1.0` (default) — `AlwaysOn`, every span is exported.
- `0.0` — `AlwaysOff`, telemetry is initialized but no spans are sent
  (useful for keeping the export pipeline warm without paying transport
  costs).
- Anything in between — `TraceIdRatioBased` sampling, e.g. `0.05` for
  5% of traces.

The sampler is set per process; downstream collectors may apply their
own sampling on top.

## Operational notes

> [!IMPORTANT]
> Spans are batched before export. A clean shutdown (SIGTERM, Ctrl-C)
> drains the queue; a hard kill loses any spans that haven't been
> flushed yet.

### What gets exported

- **Traces** — every span produced by the scanner, plus any log
  message emitted while a span is active. Those log messages ride
  along on the span and show up in your tracing UI inline with the
  span they belong to. This is the primary reason to enable telemetry.
- **Stand-alone log output** — still goes to the sink configured under
  `[log]` ([Logging](./logging.md)). Enabling telemetry does not
  silence the log sink; the same messages go to both places.
- **Metrics** — **not** exported via OTLP. Metrics are served from the
  `/metrics` endpoint in Prometheus format
  ([Server](./server.md), [Metrics](../ops/metrics.md)).
