# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in pgsense-rs, please report it privately so we can fix it before it is disclosed publicly. **Do not open a public issue.**

**Preferred channel:** [GitHub Security Advisories](https://github.com/MrEhbr/pgsense-rs/security/advisories/new) — lets us coordinate a fix and request a CVE if warranted.

If you cannot use GitHub Security Advisories, email **mr.ehbr@gmail.com** with:

- A description of the issue and its impact.
- Steps to reproduce, ideally with a minimal proof of concept.
- Affected version(s) and configuration.

We aim to acknowledge reports within 72 hours and to ship a fix within 30 days for high-severity issues. Reporters are credited in the advisory unless they prefer to remain anonymous.

## Scope

In scope:

- The `pgsense-rs` binary and library code under `src/`.
- The bundled Helm chart (`charts/`) and `Dockerfile`.
- Built-in detection rules and validators in `config/` and `src/rules/`.

Out of scope:

- Vulnerabilities in third-party dependencies — please report those upstream. We will track and roll forward.
- Misconfiguration of the user's PostgreSQL server or alert sinks.
- Findings that require pre-existing access to the host running pgsense-rs.

## Supported Versions

Only the latest minor release receives security updates. Older releases may receive critical fixes at maintainer discretion.
