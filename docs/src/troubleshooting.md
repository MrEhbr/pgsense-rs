# Troubleshooting

Common issues and where to start looking.

> [!TIP]
> Most configuration and connectivity issues are caught by
> `pgsense-rs validate -c config.toml --connect` before the scanner
> tries to start. Run it any time the scanner refuses to start or
> behaves unexpectedly — see [`pgsense-rs validate`](./cli/validate.md).

## "publication ... does not exist"

The publication referenced by `[[databases]].publication` (default
`pgsense_pub`) hasn't been created on the source database.

```sql
CREATE PUBLICATION pgsense_pub FOR ALL TABLES;
```

See [PostgreSQL Setup](./getting-started/postgres-setup.md) for granular
publication options.

## "must be superuser or have REPLICATION privilege"

The replication user doesn't have the `REPLICATION` attribute. On AWS
RDS / Aurora, you also need to grant the `rds_replication` role.

```sql
ALTER ROLE pgsense WITH REPLICATION;
-- AWS RDS / Aurora additionally:
GRANT rds_replication TO pgsense;
```

## No findings appearing

Walk the pipeline from source to alert:

1. Check `pgsense_pipeline_connected{database="..."}` — if `0`, the pipeline isn't connected. Check logs at `info` or `debug` level for the connection error.
2. Check `pgsense_events_total{database="..."}` — if not increasing, your publication doesn't include the table you're writing to, or replication isn't actually flowing.
3. Check `pgsense_events_skipped_total{reason="..."}` — your scan filter may be excluding the schema/table/column you care about.
4. Run `pgsense-rs rules test --input "<known-sensitive-value>"` — confirms the rule itself matches the value in isolation.
5. Confirm the column type is text-shaped — `int`, `bool`, `uuid`, `bytea`, `numeric`, timestamps, `interval`, etc. are skipped automatically by [column-type filtering](./configuration/scan-filter.md#column-type-filtering-always-on).
6. Check `pgsense_dedup_total{outcome="suppressed"}` — every finding may be getting deduplicated against a recent identical one. Lower `dedup_window_seconds` or insert a new distinct value.

## Pipeline keeps reconnecting

`pgsense_pipeline_reconnects_total` rising indicates the replication
connection is unstable. Common causes:

- Network drops between pgsense-rs and PostgreSQL.
- `wal_sender_timeout` shorter than your event interarrival on a quiet database.
- Cloud Postgres instance restarting (maintenance window, parameter group apply).
- Replication slot invalidated (e.g. log retention exceeded).

For idle-database timeouts, raise `wal_sender_timeout` on the server or
generate a periodic heartbeat write.

## Hot reload didn't pick up my change

- Make sure you saved the file (some editors keep changes in memory until explicitly saved).
- Check `pgsense_config_reloads_total{status="error"}` — if it's incrementing, your edit failed to compile and the previous rule set is still active. Look in the logs for the compile error.
- Editing the `.rhai` script file alone doesn't trigger a reload — touch the rules file (e.g. `touch config/rules.toml`) to pick up script edits.

## "rule routes to channel '...' which is not configured" warning

A rule's `channels` field references a name that no enabled channel
matches. The finding will be silently dropped (no channel will accept
it) until you either:

- Add the missing channel to your config.
- Remove the unknown name from the rule's `channels`.
- Rename a channel to match (custom `name = "..."` on the channel block).

Run `pgsense-rs validate -c config.toml -r rules.toml` to surface this
without starting the scanner.

## Memory growth over time

The dedup cache is soft-bounded at 10 000 entries — once that threshold
is exceeded, expired entries are pruned on the next lookup. If you
observe memory growing well beyond what the cache size implies:

- Check `pgsense_findings_total` — a runaway false-positive rule can churn through dedup keys faster than the prune fires.
- Check `pgsense_queue_depth{database}` — if rising, the scanner is falling behind replication and unprocessed batches are accumulating.

## "Cannot create slot — too many slots"

Increase `max_replication_slots` on the source server. Each
`[[databases]]` entry needs one slot.

## Source database disk fills up

A replication slot keeps WAL segments on disk until the subscriber
consumes them. If pgsense-rs is shut down without dropping its slot,
WAL on the source server accumulates until the disk fills.

> [!CAUTION]
> Always drop the slot when decommissioning a pgsense-rs deployment
> permanently:
> ```sql
> SELECT pg_drop_replication_slot('<slot_name>');
> ```
> Slot names are derived deterministically from `host/dbname` — find
> them with `SELECT slot_name FROM pg_replication_slots`.
