# `pgsense-rs scan`

Starts the scanner. Connects to every database in `[[databases]]`, opens
replication streams, and dispatches findings to configured alert channels.

## Synopsis

```
pgsense-rs scan [OPTIONS]
```

## Options

| Flag | Description |
|------|-------------|
| `-c, --config <FILE>` | Path to the configuration TOML. |
| `-r, --rules <FILE>` | Path to the rules TOML. Overrides `rules_file` from config. |
| `-v` / `-vv` / `-vvv` | Increase log verbosity (info → debug → trace). |
| `-q` / `-qq` | Decrease log verbosity. |
| `--help` | Show help text. |

## Examples

```bash
# Use the default config path
pgsense-rs scan

# Explicit config and rules
pgsense-rs scan -c /etc/pgsense/config.toml -r /etc/pgsense/rules.toml

# Trace-level logging
pgsense-rs scan -c config.toml -vvv
```

## Lifecycle

1. Load config (TOML + env overrides + file-backed secret resolution + validation).
2. Initialize the logging subsystem.
3. Initialize Prometheus metrics.
4. Spawn the optional HTTP server (when `[server] enabled = true`).
5. Compile rules (fail-fast on syntax or validator errors).
6. Build the alert dispatcher and warn about any rule that routes to a
   non-existent channel.
7. Spawn one pipeline per `[[databases]]` entry via the supervisor.
8. Subscribe to rule-file changes for hot reload.
9. Run until SIGINT / SIGTERM, then drain in-flight events and exit.

> [!TIP]
> The scanner watches the rules file for changes while running — saving
> a change recompiles the rule set in place without reconnecting any
> pipelines. See [Hot Reload](../rules/hot-reload.md).

## What's not hot-reloadable

> [!WARNING]
> The configuration file is **not** watched. Restart the process to pick
> up `config.toml` changes (database connections, alert channels, server
> port, store choice, log config).

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Clean shutdown after Ctrl+C. |
| Non-zero | Configuration failed to load, rules failed to compile, or all pipelines exited with an error. |
