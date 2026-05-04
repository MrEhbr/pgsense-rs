# Installation

pgsense-rs ships as a single binary. Pick whichever installation method
fits your environment.

## Prerequisites

- PostgreSQL **16 or newer** with logical replication enabled (`wal_level = logical`)
- For source builds: a recent stable Rust toolchain (edition 2024)

## From a release binary

Pre-built binaries for Linux and macOS are published to the
[GitHub Releases](https://github.com/MrEhbr/pgsense-rs/releases) page.

```bash
# Example for Linux x86_64 — adjust the URL for your platform
curl -sSL https://github.com/MrEhbr/pgsense-rs/releases/latest/download/pgsense-rs-x86_64-unknown-linux-gnu.tar.gz \
    | tar -xz -C /usr/local/bin
pgsense-rs --version
```

> [!TIP]
> For production, pin to a specific release tag rather than `latest` so
> you control upgrades. Configuration schema can change between minor
> versions.

## From source

```bash
git clone https://github.com/MrEhbr/pgsense-rs
cd pgsense-rs
just install                  # cargo install --path .
# or
PROFILE=release just build    # binary in target/release/pgsense-rs
```

## Docker

Multi-arch images (`linux/amd64`, `linux/arm64`) are published to GHCR:

```bash
docker pull ghcr.io/mrehbr/pgsense-rs:latest
docker run --rm ghcr.io/mrehbr/pgsense-rs:latest --help
```

## Kubernetes (Helm)

The repository includes a Helm chart at
[`charts/pgsense/`](https://github.com/MrEhbr/pgsense-rs/tree/main/charts/pgsense).
See [Helm Deployment](../ops/helm.md).

## Verify the install

```bash
pgsense-rs --version
pgsense-rs --help
```

Once you have the binary, validate a config file before running:

```bash
pgsense-rs validate -c my-config.toml --connect
```

See [`pgsense-rs validate`](../cli/validate.md) for what gets checked.
