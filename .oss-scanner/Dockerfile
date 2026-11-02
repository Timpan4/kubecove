# syntax=docker/dockerfile:1
# Keep Bun and Rust aligned with the production Dockerfile and CI.
FROM oven/bun:1.4.4@sha256:1d22548f177f4ff38e0c15baa3c2c46c6c2060ba2d1bc573bb4720b407548f1d AS bun
FROM node:26.13.0-trixie-slim@sha256:e4db4c914bf8ad241bd24804fc25d272d89bad97e68cdc8a1414862790a02c39 AS node
FROM rust:1.99.1-slim-trixie@sha256:b8e062b1c3a80183df83d9ba187cadd2b5ab84bc78d453dbfeb2fccd5f982247

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
