# Masking

When a finding is dispatched to an alert channel, the matched value is
masked — only a fingerprint of the original text leaves the scanner.
Masking is applied uniformly to every channel; there is no per-channel
override.

## Algorithm

The masker preserves the first two characters and the last two
characters of the match, replacing every character in between with `*`.
For matches shorter than 8 characters, the entire match becomes
asterisks.

| Original | Masked |
|----------|--------|
| `a` | `*` |
| `ab` | `**` |
| `abcde` | `*****` |
| `abcdefgh` | `ab****gh` |
| `1234567890` | `12******90` |
| `secretvalue` | `se*******ue` |
| `4111111111111111` | `41************11` |

> [!IMPORTANT]
> Masking is **unconditional** — there is no flag to send raw values to
> any channel. The original value never leaves the scanner process and is
> never written to logs, files, webhooks, or the PostgreSQL alert table.

## What gets sent with each finding

Every alert payload includes:

- The full database identity (`host/dbname`)
- Schema, table, and column names
- The rule ID, description, category, and severity
- The masked match (`masked_sample`)
- Primary keys for the row, with **any column that produced a match
  excluded** to avoid leaking the sensitive value via primary-key
  echo (this matters when `REPLICA IDENTITY FULL` is set on the table)
- The commit LSN

## Reproducing a finding

Because the raw value never appears in alerts, reproducing a finding
requires:

1. Looking up the row by the included primary keys.
2. Reading the column directly from the source database.

The alert alone is not enough to retrieve the sensitive value, which is
the point — alert delivery infrastructure becomes safe to share with
people who shouldn't see the raw data.
