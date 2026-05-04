# Health Endpoints

The [server](../configuration/server.md) exposes two endpoints for
container orchestrators and external health checks.

## `/health`

```
GET /health
200 OK
{"status": "ok"}
```

Always returns 200 if the HTTP listener is responding. Suitable for a
**liveness** probe — its only signal is "the process is alive enough to
answer HTTP". A failing pipeline or stuck rule engine will not flip this
to a non-200 response.

## `/ready`

```
GET /ready
200 OK   {"status": "ready"}     — supervisor has spawned the pipelines
503      {"status": "not_ready"} — startup not complete
```

Returns 200 once the supervisor has finished spawning all configured
pipelines. Suitable for a **readiness** probe.

> [!NOTE]
> `/ready` flips to 200 once pipelines have been *spawned*, not
> necessarily *connected*. A misconfigured database (wrong host, bad
> credentials) can leave a pipeline in a reconnect loop while `/ready`
> still returns 200. For per-database connection state, scrape
> `pgsense_pipeline_connected{database="..."}` from `/metrics` and
> alert on it directly.

## Kubernetes probes

```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 9090
  periodSeconds: 10
  failureThreshold: 3
readinessProbe:
  httpGet:
    path: /ready
    port: 9090
  periodSeconds: 10
  failureThreshold: 3
  initialDelaySeconds: 5
```

`initialDelaySeconds` on the readiness probe gives the supervisor time
to spawn pipelines before the orchestrator decides the pod is unready.

## Docker / Compose

```yaml
services:
  pgsense:
    image: ghcr.io/mrehbr/pgsense-rs:latest
    healthcheck:
      test: ["CMD", "curl", "-fsS", "http://localhost:9090/ready"]
      interval: 10s
      timeout: 3s
      retries: 3
      start_period: 10s
```
