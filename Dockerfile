# Dockerfile for GoReleaser builds
# Uses distroless for minimal, secure images with glibc support

FROM gcr.io/distroless/cc-debian12:latest@sha256:aa0b7af67fa8211751ea6e00baa8373ba56cc1417ffc986ec9619bd0e1556b56

# Copy the pre-built binary from goreleaser's build context
# GoReleaser organizes binaries by TARGETPLATFORM (e.g., linux/amd64, linux/arm64)
ARG TARGETPLATFORM
ARG BINARY_NAME
COPY ${TARGETPLATFORM}/${BINARY_NAME} /usr/local/bin/app

# Default detection rules
COPY config/rules.toml /etc/pgsense/rules.toml
ENV PGSENSE__RULES_FILE=/etc/pgsense/rules.toml

ENTRYPOINT ["/usr/local/bin/app"]
