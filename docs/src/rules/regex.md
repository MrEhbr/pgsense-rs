# Regex Rules

Regex is the default rule type — when `type` is omitted, the rule is treated
as a regex.

## Usage

```toml
[[rules]]
id          = "credit-card-regex"
description = "Credit card numbers (pattern + Luhn check)"
category    = "PCI_DSS"
severity    = "critical"
pattern     = '\b\d{4}[- ]?\d{4}[- ]?\d{4}[- ]?\d{4}\b'
validate    = "luhn"             # optional post-match validator
```

`pattern` is required for regex rules. Use TOML literal strings (`'...'`)
to avoid having to escape backslashes.

## Engine

Patterns are compiled once at load time. At scan time the engine first
runs the entire rule set through a combined regex set as a fast-path —
only patterns that *might* match a given value are then run individually
and (optionally) validated.

This keeps the per-value cost roughly proportional to the number of
*matching* patterns, not the total rule count.

## No lookaround

> [!WARNING]
> Lookahead and lookbehind are **not supported**. Patterns like
> `(?!...)`, `(?<=...)`, or `(?<!...)` will fail to compile and the
> entire rules-file load will be aborted.

For "match X but not when followed by Y" cases, prefer a simple pattern
with a [validator](./validators.md) function:

```toml
# WRONG — fails to compile
pattern = '\d{3}-\d{2}-\d{4}(?!\s*\$)'

# RIGHT — match shape, then validate
pattern  = '\b\d{3}-\d{2}-\d{4}\b'
validate = "ssn"
```

## Validators

`validate` runs a check on each regex match and discards the match if
the validator rejects it. Available validators:

- `luhn` — Luhn checksum (credit card numbers)
- `ssn` — US SSN structural rules
- `phone` — phone-number parsing (E.164 / `00`-prefix / NANP)
- `email` — local-part + domain + TLD structural validation
- `iban` — IBAN mod-97-10 check digit

See [Validators](./validators.md) for details.

## When to use regex over a builtin

Use regex when:

- The pattern is unique to your environment (custom IDs, internal tokens).
- A built-in detector doesn't exist for the format.

> [!TIP]
> When a built-in exists (`credit_card`, `ssn`, `phone`, `email`, `iban`),
> prefer the built-in — it will be faster and have fewer false positives
> than the equivalent regex + validator combo.
