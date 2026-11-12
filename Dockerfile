# syntax=docker/dockerfile:1
# The builder always runs on the build host's platform and cross-compiles with
# cargo-zigbuild, so multi-arch images never build under QEMU emulation.
FROM --platform=$BUILDPLATFORM rust:1.99.1-alpine AS builder
ARG TARGETARCH
RUN apk add --no-cache zig musl-dev \
 && rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl \
 && cargo install cargo-zigbuild --version 0.24.1 --locked
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/target,sharing=locked \
    set -eux; \
    case "$TARGETARCH" in \
      arm64) triple=aarch64-unknown-linux-musl ;; \
      *) triple=x86_64-unknown-linux-musl ;; \
    esac; \
    cargo zigbuild --release -p zerowol-agent --target x86_64-unknown-linux-musl; \
    cargo zigbuild --release -p zerowol-agent --target aarch64-unknown-linux-musl; \
    ZEROWOL_AGENT_X86_64=/src/target/x86_64-unknown-linux-musl/release/zerowol-agent \
    ZEROWOL_AGENT_AARCH64=/src/target/aarch64-unknown-linux-musl/release/zerowol-agent \
      cargo zigbuild --release -p zerowol --target "$triple"; \
    cp "target/$triple/release/zerowol" /zerowol

FROM scratch AS prebuilt
COPY dist/zerowol /zerowol
USER 1026:100
EXPOSE 8090
ENV ZEROWOL_DATA=/data
ENTRYPOINT ["/zerowol"]

# Bare binary, for `docker buildx build --target binary --output type=local,dest=out .`
FROM scratch AS binary
COPY --from=builder /zerowol /zerowol

FROM scratch AS runtime
LABEL org.opencontainers.image.title="zerowol" \
      org.opencontainers.image.description="Tiny Wake-on-LAN and remote shutdown server with a PWA and JSON API" \
      org.opencontainers.image.licenses="MIT"
COPY --from=builder /zerowol /zerowol
USER 1026:100
EXPOSE 8090
ENV ZEROWOL_DATA=/data
ENTRYPOINT ["/zerowol"]
