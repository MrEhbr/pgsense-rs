# Dockerfile for GoReleaser builds
# Uses distroless for minimal, secure images with glibc support

FROM gcr.io/distroless/cc-debian12:latest@sha256:847433844c7e04bcf07a3a0f0f5a8de554c6df6fa9e3e3ab14d3f6b73d780235

# Copy the pre-built binary from goreleaser's build context
# GoReleaser organizes binaries by TARGETPLATFORM (e.g., linux/amd64, linux/arm64)
ARG TARGETPLATFORM
ARG BINARY_NAME
COPY ${TARGETPLATFORM}/${BINARY_NAME} /usr/local/bin/app

# Default detection rules
COPY config/rules.toml /etc/pgsense/rules.toml
ENV PGSENSE__RULES_FILE=/etc/pgsense/rules.toml

ENTRYPOINT ["/usr/local/bin/app"]
