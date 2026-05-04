# Hot Reload

The rules file is watched for changes. Saving an edit reloads and
recompiles the rule set in place — no scanner restart, no replication
slot churn.

## What triggers a reload

Any modification (write, atomic rename, `Create` event) to the configured
rules file. Editor patterns that write through a temp file and rename
into place are handled correctly.

The watcher canonicalizes the rules-file path before subscribing and
filters events to that exact path. Events on sibling files in the same
directory are ignored.

## What the reload covers

- **Regex rules** — patterns and allowlists are recompiled.
- **Built-in rules** — toggled on/off, scope/allowlist refreshed.
- **Script rules** — the referenced `.rhai` file is re-read and re-parsed.
- All scope and channel-routing fields are rebuilt per rule.

The active rule set is swapped atomically — in-flight scans complete with
the previous set; the next event uses the new one.

## What's not hot-reloadable

> [!WARNING]
> The main configuration file (`config.toml`) is **not** watched. Changes
> to database connection details, alert channel configuration, server
> port, store choice, or any other field outside the rules file require
> a process restart.

In short: rules and their referenced scripts hot-reload; everything else
needs a restart.

## Failure handling

If the new rules file fails to parse, or any individual rule fails to
compile (invalid regex, missing builtin, broken Rhai script), the reload
is **rolled back** and the previous valid rule set stays active. The
failure is logged at `warn` and counted in
`pgsense_config_reloads_total{status="error"}`.

> [!TIP]
> You can safely edit the rules file in production — a typo won't take
> the scanner down. Watch `pgsense_config_reloads_total{status="error"}`
> in your monitoring to catch silent rollbacks where you *thought* a
> change took effect but didn't.

## Debounce

After receiving a change event, the reload waits ~100ms and drains any
queued events before recompiling, so editor patterns that emit multiple
filesystem events per save (write + rename + chmod) trigger a single
reload.

## Metrics

| Metric | Description |
|--------|-------------|
| `pgsense_config_reloads_total{status="ok"}` | Successful reloads. |
| `pgsense_config_reloads_total{status="error"}` | Failed reloads (previous rule set stays active). |
| `pgsense_rules_loaded` | Gauge of currently-loaded rule count. |
