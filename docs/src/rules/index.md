# Detection Rules

pgsense-rs evaluates every scannable text value against a set of detection
rules. Rules live in a separate TOML file (referenced via `--rules` /
`-r`) so they can be edited, hot-reloaded, and version-controlled
independently from the main configuration.

## Rule types

`Builtin`
:   The data is a well-known format (credit card, SSN, IBAN, phone,
    email). The builtin algorithmic detector is faster and more accurate
    than the equivalent regex.

`Regex`
:   You have a custom shape-based pattern, optionally combined with a
    validator (`luhn`, `ssn`, `phone`, `email`, or `iban`).

`Script`
:   You need conditional logic that regex can't express. Rhai scripts
    run in a sandboxed engine.

## Schema

```toml
[[rules]]
id          = "credit-card"      # required, unique
description = "Credit card numbers"   # required
type        = "builtin"          # "regex" (default), "builtin", or "script"
category    = "PCI_DSS"          # required, free-form, used in metrics labels
severity    = "critical"         # required: critical | high | medium | low | info
channels    = ["slack", "log"]   # optional — restrict to specific alert channels

# Type-specific fields:
builtin   = "credit_card"          # for type = "builtin"
pattern   = '\b[A-Z]{2}\d{6}\b'    # for type = "regex"
script    = "scripts/x.rhai"       # for type = "script"

# Optional refinements (regex rules only):
validate  = "luhn"                 # one of: luhn, ssn, phone, email, iban

[rules.allowlist]
values    = ["4111111111111111"]   # exact matches to ignore
patterns  = ['^4111-?']            # regex patterns to ignore

[rules.scope]
include_schemas = ["public"]
include_tables  = ["users", "orders*"]
exclude_columns = ["*_hash"]
```

> [!NOTE]
> `id`, `description`, `category`, and `severity` are all required on
> every rule. Missing any of them fails the rules-file load.

## Evaluation order

The rule engine runs three phases per scanned value:

1. **Regex** — A combined regex set is consulted as a fast-path; only
   patterns that *might* match the value are then run individually and
   (optionally) validated.
2. **Builtin** — Algorithmic detectors run with boundary-aware scanning.
3. **Script** — Rhai scripts execute against the value and return any
   matches.

Findings from all three phases are aggregated, deduplicated, and routed
to alert channels.

## Hot reload

The rules file is watched for changes. Saving an edit reloads and
recompiles the rule set in place — no scanner restart, no replication
slot churn. See [Hot Reload](./hot-reload.md) for details and failure
handling.

## Where to go next

- [Built-in Detectors](./builtin.md) — the algorithmic detector catalog
- [Regex Rules](./regex.md) — pattern syntax and gotchas (no lookahead/lookbehind)
- [Rhai Script Rules](./scripts.md) — sandbox constraints and the `detect(value)` contract
- [Validators](./validators.md) — Luhn, SSN, phone, email, IBAN
- [Allowlists & Scope](./scope.md) — taming false positives
- [Masking](./masking.md) — how matched values are obfuscated before alerting
