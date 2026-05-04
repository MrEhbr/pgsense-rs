# Stdout Channel

Writes a human-readable line per finding directly to stdout, bypassing
the logger.

## Configuration

```toml
[alerts.stdout]
enabled = false   # default false
```

The channel is disabled by default.

## When to use

- Quick local exploration: `pgsense-rs scan -c config.toml | grep ...`
- Pipeline-style integration where another process consumes stdout.
- Demos and screencasts where you want clean, unstructured output.

> [!WARNING]
> Enabling `stdout` and a `console`-format `log` output (default) at the
> same time interleaves finding lines with regular log output. For
> production use the [Log Channel](./log.md) with `format = "json"`.

## Format

Each line is a human-readable summary including the database identity,
table, column, rule, and the masked value. The exact format is not
stable across versions — don't parse it programmatically. Use the
[JSONL Channel](./jsonl.md) if you need a stable machine-readable
representation.
