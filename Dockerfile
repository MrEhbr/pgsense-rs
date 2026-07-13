# Dockerfile for GoReleaser builds
# Uses distroless for minimal, secure images with glibc support

FROM gcr.io/distroless/cc-debian12:latest@sha256:a90cf0f046efb32466b38b0972fef3a95e7c580e392e79ff1b7ac08c15fed0bc

# Copy the pre-built binary from goreleaser's build context
# GoReleaser organizes binaries by TARGETPLATFORM (e.g., linux/amd64, linux/arm64)
ARG TARGETPLATFORM
ARG BINARY_NAME
COPY ${TARGETPLATFORM}/${BINARY_NAME} /usr/local/bin/app

# Default detection rules
COPY config/rules.toml /etc/pgsense/rules.toml
ENV PGSENSE__RULES_FILE=/etc/pgsense/rules.toml

ENTRYPOINT ["/usr/local/bin/app"]
