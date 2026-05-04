# Allowlists & Scope

Two per-rule blocks let you tame false positives and target a rule at the
data you care about: `[rules.allowlist]` for value-level exceptions, and
`[rules.scope]` for restricting where the rule runs.

## Allowlist

The allowlist filters out matches *after* they're produced by the regex,
built-in detector, or script. If a match equals an allowlisted value or
matches an allowlisted pattern, it's silently dropped.

```toml
[[rules]]
type        = "builtin"
id          = "email-address"
description = "Email addresses (production)"
category    = "PII"
severity    = "high"
builtin     = "email"

[rules.allowlist]
description = "System and test addresses"
values = ["noreply@example.com", "no-reply@example.com"]
patterns = [
    '.*@example\.com$',
    '.*@test\.com$',
    '^noreply@',
    '^postmaster@',
]
```

| Field | Effect |
|-------|--------|
| `values` | Exact-match list — the matched substring must equal an entry. |
| `patterns` | Regex patterns; if any pattern matches the matched substring, the finding is suppressed. |
| `description` | Free-form text, included in compile-time logging. |

## Scope

Scope limits *where* the rule runs in terms of schemas, tables, and
columns. Unlike the global [scan filter](../configuration/scan-filter.md),
scope is per-rule.

```toml
[[rules]]
type        = "builtin"
id          = "ssn-users-only"
description = "SSN detection in user-facing tables only"
category    = "PII"
severity    = "critical"
builtin     = "ssn"

[rules.scope]
include_tables  = ["users", "employee*"]   # exact + glob
exclude_columns = ["*_hash"]
```

All scope fields support exact strings and glob patterns (`*`, `?`):

| Field | Effect |
|-------|--------|
| `include_schemas` | If non-empty, only run in these schemas. |
| `include_tables` | If non-empty, only run in these tables. |
| `exclude_tables` | Skip these tables. |
| `include_columns` | If non-empty, only run on these columns. |
| `exclude_columns` | Skip these columns. |

> [!IMPORTANT]
> A table or column listed in *both* `include_*` and `exclude_*` is
> rejected at rules-file load — the engine refuses to compile a rule
> with that ambiguity. Pick one or the other.

## Allowlist + scope interaction

The two are independent. Scope decides *whether* the rule runs at all on a
given column; the allowlist filters individual matches. They compose
naturally:

```toml
[rules.scope]
include_schemas = ["public"]
exclude_tables  = ["audit_*"]

[rules.allowlist]
patterns = ['^test_.*@example\.com$']
```
