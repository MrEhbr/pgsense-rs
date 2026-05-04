# Validators

A validator is an optional post-match check applied to regex rules. After a
regex matches, the validator runs against the matched substring; if it
returns false, the match is discarded as a false positive.

## Available validators

`luhn`
:   Luhn (mod-10) checksum — used for credit card numbers and many other
    identifier schemes. Strips non-digit characters before checking;
    requires at least 13 digits and at most 19.

`ssn`
:   US SSN structural rules. Accepts `XXX-XX-XXXX`, `XXX XX XXXX`, and
    `XXX.XX.XXXX` formats. Rejects area `000`, `666`, or `>= 900`,
    group `00`, and serial `0000`.

`phone`
:   Parse-and-validate via a phone-number library. Handles E.164
    (`+` prefix), `00` international dial prefix, and bare NANP
    numbers (defaulted to `US`).

`email`
:   Structural validation of local part (1–64 chars, RFC-5321 dot-atom
    charset, no leading/trailing/consecutive dots), domain (labels of
    1–63 alphanumeric/hyphen chars), and TLD (≥ 2 alphabetic chars,
    punycode rejected).

`iban`
:   ISO 7064 mod-97-10 check on the rearranged, letter-translated
    digits. Strips spaces and dashes before validating.

Setting any other value fails the rules-file load.

## Usage

```toml
[[rules]]
id          = "credit-card-regex"
description = "Credit card numbers (with Luhn check)"
category    = "PCI_DSS"
severity    = "critical"
pattern     = '\b\d{4}[- ]?\d{4}[- ]?\d{4}[- ]?\d{4}\b'
validate    = "luhn"
```

## Why validators exist

The regex engine has no lookaround, so context-sensitive false-positive
filtering must happen outside the regex. Validators run in compiled
Rust code with no per-rule compilation cost.

> [!TIP]
> Built-in detectors (`type = "builtin"`) already include the appropriate
> validator algorithm internally — there is no need to set `validate` on
> a builtin rule. The field is silently ignored if you do.

## Picking the right validator

| Pattern matches… | Use… |
|------------------|------|
| Credit card shapes | `validate = "luhn"` (or use `builtin = "credit_card"`) |
| SSN shapes | `validate = "ssn"` (or use `builtin = "ssn"`) |
| Phone numbers | `validate = "phone"` (or use `builtin = "phone"`) |
| Emails | `validate = "email"` (or use `builtin = "email"`) |
| IBANs | `validate = "iban"` (or use `builtin = "iban"`) |

> [!TIP]
> If a built-in detector exists for what you're trying to match, prefer
> the built-in. Built-in detectors include the appropriate validator
> internally *and* use boundary-aware scanning that finds matches
> inside larger text — a regex-plus-validator combo only works if your
> pattern already isolates the candidate substring.
