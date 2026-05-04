# Built-in Detectors

Built-in detectors are algorithmic, hand-tuned scanners for well-known
sensitive-data formats. They are faster and more accurate than equivalent
regex rules — most rules should use a builtin when one is available.

## Available detectors

`credit_card`
:   Credit card numbers (Visa, Mastercard, Amex, Discover) with Luhn
    validation.

`ssn`
:   US Social Security Numbers, with structural and area-number
    validation.

`phone`
:   Phone numbers in E.164, NANP, and international `00`-prefix formats.

`email`
:   RFC-5322-shaped email addresses.

`iban`
:   International Bank Account Numbers, with mod-97 checksum.

## Usage

```toml
[[rules]]
type        = "builtin"
id          = "credit-card"
description = "Credit card numbers (Visa, MC, Amex, Discover)"
category    = "PCI_DSS"
severity    = "critical"
builtin     = "credit_card"
```

> [!IMPORTANT]
> `type = "builtin"` is required — without it, the rule is treated as a
> regex and `pattern` becomes mandatory. The `builtin` field is also
> required for builtin rules; missing it aborts the rules-file load.

## Boundary-aware matching

Built-in detectors scan inside arbitrary text rather than requiring the
entire field to match. They use boundary heuristics (whitespace,
punctuation, end-of-string) to avoid splitting numbers or capturing
neighboring characters.

This means a column containing free-form text like
`"Customer card on file: 4111-1111-1111-1111 (visa)"` produces one
clean finding rather than several partial matches.

## Combining with allowlists and scope

Built-in rules accept the same `[rules.allowlist]` and `[rules.scope]`
blocks as any other rule type — see [Allowlists & Scope](./scope.md).

```toml
[[rules]]
type        = "builtin"
id          = "email-prod"
description = "Production-only email detection"
category    = "PII"
severity    = "high"
builtin     = "email"

[rules.allowlist]
patterns = ['.*@example\.com$', '^noreply@', '^postmaster@']

[rules.scope]
exclude_tables = ["audit_*"]
```
