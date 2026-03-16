APP := `basename $(pwd)`
profile := env_var_or_default('PROFILE', 'debug')

# Show available targets
help:
    @just --list

# Install development tools (cargo plugins)
setup:
    @echo "Installing development tools..."
    @cargo install --locked prek
    @prek install
    @cargo install cargo-llvm-cov --locked
    @cargo install cargo-afl
    @cargo afl config --build --force
    @echo "✓ Development tools installed"

# Rust
# Build application binary
build *opts="":
    @echo "Building {{APP}} ({{profile}} profile)"
    @cargo build {{ if profile == "release" { "--release" } else { "" } }} {{opts}}

# Install application into ~/.cargo/bin
install *opts="":
    @echo "Installing {{APP}}"
    @cargo install --path . {{opts}}

# Run tests
test *opts="--workspace":
    @cargo nextest run {{opts}}

# Generate code coverage report (requires: cargo install cargo-llvm-cov)
test-coverage *opts="--workspace":
    @cargo llvm-cov nextest {{opts}}

# Run documentation tests
test-doc *opts="--workspace":
    @cargo test --doc {{opts}}

# Lint code
lint *opts="":
    cargo clippy --workspace --fix --allow-dirty --allow-staged --no-deps --all-targets --all-features {{opts}} -- -D warnings
    @cargo fmt --all -- --check

# Format code
fmt:
    @cargo fmt --all

# Check code for typos
typos:
    @typos --write-changes

# Tidy dependencies
tidy:
    @cargo update

# Download dependencies
deps:
    @cargo fetch

# Run benchmarks (skip unit tests, filter outlier messages)
bench *opts="":
    @cargo criterion --workspace --output-format criterion {{opts}}

# Run a fuzz target
[arg("duration", short="d", long="duration")]
fuzz target duration="0":
    cd fuzz && cargo afl build
    mkdir -p fuzz/output/{{target}}
    cd fuzz && cargo afl fuzz \
        -i corpus/{{target}} \
        -o output/{{target}} \
        {{ if duration != "0" { "-V " + duration } else { "" } }} \
        target/debug/{{target}}

# Run application
run +args="--help":
    @cargo run {{ if profile == "release" { "--release" } else { "" } }} -- {{args}}

### Dev targets

# Run dev environment (--profile bench for observability stack)
dev *opts:
    @docker compose -f .docker/docker-compose.yaml {{opts}} up --wait

# Stop dev environment
dev-stop *opts:
    @docker compose -f .docker/docker-compose.yaml {{opts}} stop

# Remove dev environment (volumes + orphans)
dev-clean *opts:
    @docker compose -f .docker/docker-compose.yaml {{opts}} down -v --remove-orphans

### Bench targets

# Run pgbench (sensitive=0..100 tps=500 duration=60 clients=4)
pgbench sensitive="10" tps="500" duration="60" clients="4":
    @PGPASSWORD=postgres pgbench \
        -f .docker/pgbench/clean.sql@$((100 - {{sensitive}})) \
        -f .docker/pgbench/sensitive.sql@{{sensitive}} \
        -h localhost -p ${PG_PORT:-5432} -U postgres \
        -c {{clients}} -j 2 -T {{duration}} -R {{tps}} -P 5 \
        postgres
