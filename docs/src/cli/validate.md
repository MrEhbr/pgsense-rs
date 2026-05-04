# `pgsense-rs validate`

Validates the configuration without starting any pipelines. Optionally
tests live connectivity to source databases and alert endpoints.

## Synopsis

```
pgsense-rs validate -c <CONFIG> [-r <RULES>] [--connect]
```

## Options

| Flag | Description |
|------|-------------|
| `-c, --config <FILE>` | **Required.** Path to the configuration TOML. |
| `-r, --rules <FILE>` | Optional. Path to the rules TOML. Overrides `rules_file` from config. |
| `--connect` | Also test live connectivity to all databases and alert endpoints. |

## What it checks

In offline mode (no `--connect`):

- **config** — TOML parses, env overrides apply, `password_file` resolves,
  cross-field validation passes.
- **databases** — every `[[databases]]` entry has non-empty
  `host`/`dbname`/`username`/`publication`, port is non-zero, and no two
  entries share an identity (`host/dbname`).
- **store** — Reports the configured store backend (`memory` or
  `postgres`).
- **rules** — Compiles every rule and reports per-type counts. Warns
  for any rule that routes to a `channels` name that no enabled channel
  matches.
- **alerts** — Validates each enabled channel's config (URL scheme for
  webhooks, non-empty token/channel for Slack, identifier validity for
  Postgres schema/table) and warns on duplicate channel names.

With `--connect`, additionally:

- Opens a TCP connection to each database and runs a `SELECT 1` to
  verify credentials.
- Sends a HEAD request to each webhook URL.
- Calls Slack's `auth.test` for each Slack token.
- Connects to the Postgres alert store and runs `SELECT 1` to verify
  credentials and reachability.

## Output

Each check produces a tagged line:

```
[OK]    config: parsed successfully (2 databases, store=postgres)
[OK]    databases: 2 database(s) configured (primary/orders, replica/users)
[OK]    store: postgres (state persisted in source DB under `etl` schema)
[OK]    rules: 18/18 rules compiled (12 regex, 5 builtin, 1 script)
[WARN]  rules: rule 'old-pattern' routes to channel 'siem-old' which is not configured
[OK]    alerts: 3 channels configured (log, postgres, siem-new)

Validation complete: 0 errors, 1 warnings
```

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | No errors (warnings allowed). |
| Non-zero | One or more checks reported an error. |

> [!TIP]
> Run `pgsense-rs validate -c config.toml --connect` in CI before
> deploying — it catches most "deploy fails because of bad TLS cert /
> wrong publication / dropped Slack token" issues before they hit
> production. Without `--connect`, the same command makes a fast,
> deterministic check that's safe to run on every pull request.
