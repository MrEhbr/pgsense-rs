# Contributing

Thanks for your interest in pgsense-rs.

## Development setup

The project uses Nix for a pinned toolchain. With direnv:

```bash
direnv allow
```

Or directly:

```bash
nix develop
```

The dev shell provides the Rust toolchain, `just`, `cargo-nextest`, `mdbook`, Docker tooling, and `prek`.

## Common tasks

```bash
just build         # debug build (PROFILE=release for release)
just test          # full test suite (Docker required for integration tests)
just lint          # clippy + rustfmt check
just fmt           # apply formatting
just bench         # criterion benchmarks
just docs-serve    # live preview the mdBook docs
```

## Tests

Integration tests use `testcontainers` and require a Docker daemon. On macOS with Colima:

```bash
export DOCKER_HOST=unix:///$HOME/.colima/default/docker.sock
just test
```

## Commit messages

We use [Conventional Commits](https://www.conventionalcommits.org/). Allowed types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`. The changelog is generated from these via `git-cliff`.

Examples:

```
feat(rules): add IBAN country-code allowlist
fix(pipeline): retry table sync on transient connection error
```

Keep structural changes (rename, move, format) separate from behavioral changes.

## Pre-commit hooks

```bash
prek install
```

This runs format checks, typo checks, and basic lints on every commit.

## Pull requests

- Branch from `main`.
- Keep PRs focused — one logical change per PR.
- Ensure `just test` and `just lint` pass.
- Update docs under `docs/src/` when you change user-visible behavior.
- Add or update tests for new logic.

## Reporting bugs and proposing features

Use the GitHub issue templates. For security issues, see [SECURITY.md](SECURITY.md) — do not file public issues.

## License

By contributing, you agree your contributions are licensed under the MIT License (see [LICENSE](LICENSE)).
