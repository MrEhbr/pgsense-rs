# Dockerfile for GoReleaser builds
# Uses distroless for minimal, secure images with glibc support

FROM gcr.io/distroless/cc-debian12:latest@sha256:e5d81ddde149641e2a9ba55be4545bc125c67de07508b03ba4c22e6eb0ded5aa

# Copy the pre-built binary from goreleaser's build context
# GoReleaser organizes binaries by TARGETPLATFORM (e.g., linux/amd64, linux/arm64)
ARG TARGETPLATFORM
ARG BINARY_NAME
COPY ${TARGETPLATFORM}/${BINARY_NAME} /usr/local/bin/app

# Default detection rules
COPY config/rules.toml /etc/pgsense/rules.toml
ENV PGSENSE__RULES_FILE=/etc/pgsense/rules.toml

ENTRYPOINT ["/usr/local/bin/app"]
