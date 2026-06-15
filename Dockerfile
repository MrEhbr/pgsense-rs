# Dockerfile for GoReleaser builds
# Uses distroless for minimal, secure images with glibc support

FROM gcr.io/distroless/cc-debian12:latest@sha256:d703b626ba455c4e6c6fbe5f36e6f427c85d51445598d564652a2f334179f96e

# Copy the pre-built binary from goreleaser's build context
# GoReleaser organizes binaries by TARGETPLATFORM (e.g., linux/amd64, linux/arm64)
ARG TARGETPLATFORM
ARG BINARY_NAME
COPY ${TARGETPLATFORM}/${BINARY_NAME} /usr/local/bin/app

# Default detection rules
COPY config/rules.toml /etc/pgsense/rules.toml
ENV PGSENSE__RULES_FILE=/etc/pgsense/rules.toml

ENTRYPOINT ["/usr/local/bin/app"]
