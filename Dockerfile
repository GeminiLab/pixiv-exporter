FROM rust:1.91.0-bullseye AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends musl-tools upx-ucl \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app
COPY . .

# Extract version from Cargo.toml for image metadata/artifacts.
RUN APP_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)" \
    && test -n "${APP_VERSION}" \
    && printf '%s\n' "${APP_VERSION}" > /tmp/pixiv-exporter-version

RUN cargo build --profile release-musl --target x86_64-unknown-linux-musl

# Normalize output location for downstream tooling.
RUN upx --best --lzma target/x86_64-unknown-linux-musl/release-musl/pixiv-exporter -o /app/pixiv-exporter

FROM alpine:latest AS certs

RUN apk --no-cache add ca-certificates

FROM scratch

COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /app/pixiv-exporter /pixiv-exporter
COPY --from=builder /tmp/pixiv-exporter-version /VERSION

ENTRYPOINT ["/pixiv-exporter"]
