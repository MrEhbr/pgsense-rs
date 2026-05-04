# Quick Start

This walkthrough gets pgsense-rs scanning a local PostgreSQL database
in about five minutes.

## 1. Enable logical replication

In `postgresql.conf`:

```
wal_level = logical
```

> [!IMPORTANT]
> Changing `wal_level` requires a PostgreSQL restart, not just a
> `pg_reload_conf()`. Plan a maintenance window.

After restart, create a publication for the tables you want to monitor:

```sql
CREATE PUBLICATION pgsense_pub FOR ALL TABLES;
```

A more granular setup (specific tables, role permissions, replication
slots) is covered in [PostgreSQL Setup](./postgres-setup.md).

## 2. Configure pgsense-rs

Copy the bundled example and edit it to match your environment:

```bash
cp config/config.toml my-config.toml
```

At minimum, set one `[[databases]]` block with your connection details
and pick at least one alert channel. The full reference is in
[Configuration](../configuration/index.md).

> [!TIP]
> Run `pgsense-rs validate -c my-config.toml` to check your config for
> typos and missing fields before starting the scanner. Add `--connect`
> to also test live connectivity.

## 3. Add detection rules

Rules live in a separate TOML file. Start from the bundled
`config/rules.toml`, which has examples of every rule type.

```toml
# Builtin algorithmic detector — best for credit cards, SSNs, IBANs
[[rules]]
type        = "builtin"
id          = "credit-card"
description = "Credit card numbers"
builtin     = "credit_card"
category    = "PCI_DSS"
severity    = "critical"

# Regex with optional validator — good for shape-based patterns
[[rules]]
id          = "email-address"
description = "Email addresses"
pattern     = '[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}'
category    = "PII"
severity    = "high"
```

See [Detection Rules](../rules/index.md) for the full rule schema.

## 4. Run the scanner

```bash
pgsense-rs scan -c my-config.toml -r config/rules.toml
```

Findings will appear in your configured alert channels. To check rule
loading without scanning:

```bash
pgsense-rs rules list -r config/rules.toml
pgsense-rs rules test -r config/rules.toml --input "4111111111111111"
```

## 5. Verify alerts are flowing

Insert a test row that matches one of your rules:

```sql
INSERT INTO some_table (notes) VALUES ('contact: jane@example.com');
```

You should see a finding in the configured alert channel within a few
seconds.

> [!NOTE]
> If no finding appears, check the `pgsense_events_total` and
> `pgsense_events_skipped_total` metrics (when `[server] enabled = true`),
> or run with `-vvv` to see per-event scanner activity. The
> [Troubleshooting](../troubleshooting.md) page has a checklist.
