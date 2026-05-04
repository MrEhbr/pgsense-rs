# Logging

pgsense-rs emits structured logs. Output level, format, and destination are
configured under `[log]`.

## Fields

```toml
[log]
level  = "info"       # trace | debug | info | warn | error
format = "console"    # "console" | "json"
output = "stderr"     # "stderr" | "stdout" | file path
```

`level`
:   Default `info`. Standard log levels. The `RUST_LOG` env var
    overrides this with directive syntax (per-module filtering).

`format`
:   Default `console` (human-readable). Set to `json` for one JSON object
    per line, suitable for log shippers.

`output`
:   Default `stderr`. Accepts `stderr`, `stdout`, or an absolute/relative
    file path. File output is non-blocking and ANSI colors are stripped.

## Verbosity flag

The CLI accepts `-v` / `-vv` / `-vvv` to bump the level at runtime
without editing config, and `-q` / `-qq` to lower it:

```bash
pgsense-rs scan -c config.toml -vvv   # trace
pgsense-rs scan -c config.toml -q     # warn
```

CLI verbosity wins over the config file `level`.

## Per-module filtering

`RUST_LOG` overrides the configured level using the standard log
filter syntax (`module=level`, comma-separated):

```bash
RUST_LOG=pgsense_rs::pipeline=debug,pgsense_rs::rules=trace pgsense-rs scan -c config.toml
```

> [!NOTE]
> Several noisy internal modules (HTTP, TLS, async runtime, the `etl*`
> family) are pre-pinned to `warn` so they don't drown out application
> logs at `debug` or `trace`. To dig into HTTP or TLS issues with a
> webhook or Slack endpoint, raise their levels explicitly:
> ```bash
> RUST_LOG=info,hyper=debug,rustls=debug pgsense-rs scan -c config.toml
> ```

## JSON format

```toml
[log]
format = "json"
output = "/var/log/pgsense.log"
```

Each line is a JSON object with `timestamp`, `level`, `target`, `message`,
and any structured fields attached by the call site.

## Alerts vs logs

Findings are dispatched through the [Alert Channels](../alerts/index.md)
subsystem, not the logger. The `log` alert channel writes findings as
structured events that flow through the same logger; if you disable
the `log` alert channel, findings will not appear in your logs even if
log level is `info`.
