# Helm Deployment

A Helm chart for Kubernetes deployment lives in
[`charts/pgsense/`](https://github.com/MrEhbr/pgsense-rs/tree/main/charts/pgsense)
in the source repository.

## Layout

The chart renders the standard set of resources for a long-running
service:

- A workload resource running `ghcr.io/mrehbr/pgsense-rs:<tag>`.
- A `ConfigMap` holding the rendered `config.toml` and `rules.toml`.
- `Secret` mounts for any `password_file`-referenced credentials.
- An optional `Service` exposing the health/metrics port.
- Optional `ServiceMonitor` for Prometheus Operator scraping.

## Install

```bash
git clone https://github.com/MrEhbr/pgsense-rs
cd pgsense-rs

helm install pgsense ./charts/pgsense \
    --namespace pgsense \
    --create-namespace \
    --values my-values.yaml
```

## Values

The chart's `values.yaml` mirrors the structure of `config.toml`. See
`charts/pgsense/values.yaml` in the repository for the full set of
options and defaults — the chart is the source of truth, this page only
sketches the shape.

A minimal `my-values.yaml`:

```yaml
image:
  # Defaults to chart appVersion; override only if you need a specific tag.
  tag: ""

databases:
  - name: app
    host: "primary.db.svc.cluster.local"
    port: 5432
    dbname: "app"
    username: "pgsense"
    publication: "pgsense_pub"
    # Reference an existing Kubernetes Secret containing the password.
    # The chart mounts it as a file and wires `password_file` automatically.
    passwordSecret:
      name: "pgsense-db-credentials"
      key: "password"

pipeline:
  store: postgres

alerts:
  log:
    enabled: true

server:
  enabled: true
  port: 9090

serviceMonitor:
  enabled: true
```

## Secrets

> [!IMPORTANT]
> Database and Postgres-alert passwords are supplied via
> `passwordSecret: { name, key }`, referencing an existing Kubernetes
> `Secret`. The chart mounts the secret as a file and points the
> generated `config.toml` at it via `password_file`. This avoids putting
> plaintext credentials in the `ConfigMap` and avoids env-var
> inheritance leaking secrets into child processes.

## Observability

The chart exposes three independent observability surfaces, all
disabled by default:

```yaml
service:
  type: ClusterIP   # Service in front of the metrics/health port
  port: 9090

serviceMonitor:
  enabled: false    # Prometheus Operator ServiceMonitor for /metrics
  interval: 30s
  labels: {}        # extra labels (typically the Prometheus selector label)

grafanaDashboard:
  enabled: false    # ConfigMap carrying a sidecar-discoverable dashboard
  labels: {}        # selector labels for the Grafana dashboard sidecar
  annotations: {}

telemetry:
  enabled: false                     # OTLP tracing exporter
  endpoint: "http://localhost:4317"
  protocol: grpc                     # "grpc" | "http"
  serviceName: pgsense
  sampleRate: 1.0

profiling:
  enabled: false                     # per-rule + per-phase scan histograms
```

`service` is required for `serviceMonitor` to scrape the pod. The
`telemetry` and `profiling` sections render straight into the same
`config.toml` blocks documented in
[Telemetry](../configuration/telemetry.md) and
[Profiling](../configuration/profiling.md).

> [!NOTE]
> `serviceMonitor.labels` typically needs to match your Prometheus
> Operator's `serviceMonitorSelector`. `grafanaDashboard.labels`
> likewise needs to match the Grafana sidecar's
> `dashboardLabel`/`folderAnnotation` configuration. Without those, the
> objects are created but never picked up.

## Upgrade

```bash
helm upgrade pgsense ./charts/pgsense \
    --namespace pgsense \
    --values my-values.yaml
```

A `helm upgrade` triggers a rolling restart.

> [!CAUTION]
> With `pipeline.store = memory` (default), events written during the
> restart window are missed because the new pod starts from PostgreSQL's
> current LSN. Use `store = postgres` in any environment where missed
> findings would matter.
