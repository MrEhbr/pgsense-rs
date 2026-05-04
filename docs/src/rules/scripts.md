# Rhai Script Rules

When regex and built-in detectors aren't expressive enough, drop into
[Rhai](https://rhai.rs/) — a small embedded scripting language that
runs in a sandboxed engine.

## Usage

```toml
[[rules]]
type        = "script"
id          = "custom-detector"
description = "Custom detection logic"
category    = "CUSTOM"
severity    = "medium"
script      = "scripts/my_detector.rhai"
```

`script` must point to a `.rhai` file readable by the scanner. The path
is resolved relative to the working directory when pgsense-rs is launched.

## The `detect` contract

Each script must define a top-level function named `detect`:

```rhai
// scripts/my_detector.rhai
fn detect(value) {
    let matches = [];
    if value.contains("INTERNAL-") && value.len > 10 {
        matches.push(value);
    }
    matches
}
```

- Takes one argument: `value` (the column text being scanned).
- Returns an array of strings — each entry is one finding (typically the
  matched substring).
- Return an empty array `[]` to indicate "no match".

> [!IMPORTANT]
> Scripts are validated at load time with a dry-run call to
> `detect("")`. If the function is missing, takes the wrong number of
> parameters, or returns a non-string-array, the rules-file load is
> aborted and the previous rule set stays active.

## Sandbox limits

Scripts run in a sandboxed engine with the following hard caps per
invocation:

- **Operations**: 100 000
- **Call levels** (recursion depth): 16
- **Expression depth**: 64
- **Max string size**: 10 000
- **Max array size**: 1 000

> [!WARNING]
> A script that hits any of these limits is aborted, the rule is counted
> in `pgsense_script_errors_total{rule_id="..."}`, and scanning continues
> for that value with the script producing no findings. An unconditional
> infinite loop in `detect()` is caught at load time by the dry-run
> (which calls `detect("")`); a loop gated on non-empty input may instead
> be caught at runtime by the operations cap.

The sandbox is also pure-computation only — there is no filesystem,
network, or process access available to scripts.

## Hot reload

Scripts referenced from the rules file are recompiled whenever the rules
file is edited — see [Hot Reload](./hot-reload.md). Editing the `.rhai`
script file alone (without touching the rules file) does **not** trigger
a reload; touch the rules file (or save it without changes) to pick up
script edits.

## Error handling

Compile errors at load time abort the rules-file reload. Runtime errors
during `detect()` are caught, logged at `warn`, counted in
`pgsense_script_errors_total`, and treated as "no findings" for that
value. They do not crash the scanner.
