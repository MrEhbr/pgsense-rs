# Server

The optional HTTP server exposes operational endpoints — health probes for
orchestrators and Prometheus metrics for monitoring.

## Fields

```toml
[server]
enabled = false   # set true to start the server
port    = 9090
```

When `enabled = false` (default), no HTTP listener is started. Set it to
`true` in any production-like environment.

> [!IMPORTANT]
> The server binds to `0.0.0.0:<port>`. Run pgsense-rs on a private
> network or behind a firewall — neither `/metrics` nor `/health` requires
> authentication, and `/metrics` includes operationally sensitive labels
> (database identities, rule IDs).

## Endpoints

`GET /health`
:   Always returns `{"status": "ok"}` with HTTP 200 if the listener is alive.
    Liveness probe.

`GET /ready`
:   Returns `{"status": "ready"}` with HTTP 200 once at least one pipeline
    has been spawned; otherwise `{"status": "not_ready"}` with HTTP 503.
    Readiness probe.

`GET /metrics`
:   Prometheus text format (`text/plain; version=0.0.4`). See [Metrics](../ops/metrics.md).

## Kubernetes probes

```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 9090
readinessProbe:
  httpGet:
    path: /ready
    port: 9090
  initialDelaySeconds: 5
```

> [!NOTE]
> `/ready` flips to 200 once the supervisor has spawned the first pipeline
> — it does **not** wait for the replication connection itself to be
> established. For per-database connection state, scrape
> `pgsense_pipeline_connected{database="..."}` from `/metrics`.
