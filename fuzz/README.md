# Fuzz Testing

AFL-based fuzz tests for pgsense-rs builtin detectors. These verify that detector invariants hold for arbitrary input — if `scan()` returns a match, the match satisfies all documented constraints.

## Targets

| Target | Detector | Invariants checked |
|--------|----------|--------------------|
| `credit_cards` | `Detector::CreditCard` | Luhn valid, digits + separators only, 13-19 digits |
| `ssns` | `Detector::Ssn` | SSN valid, 11 chars, consistent separators (`-` ` ` `.`) |
| `phones` | `Detector::Phone` | 7-15 digits, valid phone chars only, correct start char |

## Running

```bash
just fuzz credit_cards        # run indefinitely (ctrl-c to stop)
just fuzz ssns -d 60          # run for 60 seconds
```

Requires `cargo-afl`:

```bash
cargo install cargo-afl
```

## Corpus

Seed inputs live in `corpus/<target>/`. AFL mutates these to explore new code paths.

## Output

Results go to `output/<target>/default/`:

- `crashes/` — inputs that triggered assertion failures (bugs)
- `hangs/` — inputs that caused timeouts
- `queue/` — inputs that discovered new coverage paths
- `fuzzer_stats` — execution stats (execs/sec, paths found, etc.)

No crashes = invariants hold under random input.
