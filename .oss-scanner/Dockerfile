# syntax=docker/dockerfile:1
# Keep Bun and Rust aligned with the production Dockerfile and CI.
FROM oven/bun:1.4.0@sha256:5ff609364c049b54eb0ff560ec96319729a972078ef2c755d758f0c6ef89c2d6 AS bun
FROM node:24.22.0-trixie-slim@sha256:7f486cabe892bff8fef04c0f7da8dba84a560be3e4e2740b7a86afc57ebec315 AS node
FROM rust:1.99.0-slim-trixie@sha256:2752b332db73fdbb7dc576f06c82ed1f312005784ef913d7e04a28f5f55dc581

RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential ca-certificates curl git pkg-config \
      dbus dbus-x11 gnome-keyring libayatana-appindicator3-dev libsecret-1-0 \
      libwebkit2gtk-4.1-dev librsvg2-dev libssl-dev patchelf xvfb xauth \
    && rm -rf /var/lib/apt/lists/*
COPY --from=bun /usr/local/bin/bun /usr/local/bin/bun
COPY --from=node /usr/local/bin/node /usr/local/bin/node

# OSS Scanner's working-directory contract. Keep dependencies and targets in the image,
# rather than BuildKit cache mounts, so changed reproducers can compile offline.
WORKDIR /src
COPY . /src
RUN bun install --frozen-lockfile \
    && cargo fetch --locked --manifest-path src-tauri/Cargo.toml \
    && cargo fetch --locked --manifest-path src-tauri/server/Cargo.toml

ENV CARGO_NET_OFFLINE=true
RUN --network=none bun run build \
    && cargo build --locked --manifest-path src-tauri/Cargo.toml \
    && cargo test --locked --workspace --manifest-path src-tauri/Cargo.toml --no-run
RUN --network=none KUBECOVE_PUBLIC_RUNTIME=web bun run build \
    && cargo build --locked --manifest-path src-tauri/server/Cargo.toml \
    && cargo test --locked --manifest-path src-tauri/server/Cargo.toml --no-run

CMD ["bash"]
