# syntax=docker/dockerfile:1
FROM oven/bun:1.4.0-slim@sha256:e0ee68d16ccb9927bf02aa7dd8fd4bf3369ee6d46da04faa72b05ce8bfd135f6 AS frontend
WORKDIR /source
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile
COPY . .
RUN KUBECOVE_PUBLIC_RUNTIME=web bun run build

FROM rust:1.99.0-slim-trixie@sha256:2752b332db73fdbb7dc576f06c82ed1f312005784ef913d7e04a28f5f55dc581 AS backend
ARG TARGETARCH
WORKDIR /source
COPY src-tauri/ src-tauri/
RUN case "$TARGETARCH" in \
      amd64) target=x86_64-unknown-linux-gnu ;; \
      arm64) target=aarch64-unknown-linux-gnu ;; \
      *) exit 1 ;; \
    esac \
    && cp "src-tauri/bin/kubeconform-$target" /kubeconform \
    && install -d -m 0700 /runtime-data/kubecove
ENV CARGO_PROFILE_RELEASE_STRIP=debuginfo

FROM backend AS backend-slim
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,id=kubecove-server-${TARGETARCH},target=/source/src-tauri/server/target,sharing=locked \
    cargo build --release --locked --no-default-features --manifest-path src-tauri/server/Cargo.toml \
    && cp src-tauri/server/target/release/kubecove-server /kubecove-server

FROM backend AS backend-full
COPY --from=frontend /source/dist/ dist/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,id=kubecove-server-${TARGETARCH},target=/source/src-tauri/server/target,sharing=locked \
    cargo build --release --locked --manifest-path src-tauri/server/Cargo.toml \
    && cp src-tauri/server/target/release/kubecove-server /kubecove-server

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:e792ab3d241a468a4fd7519ddbbebe66b49b5f365771716ea688ad40b6c6f1c2 AS runtime
ARG VERSION
ARG REVISION
LABEL org.opencontainers.image.source="https://github.com/Timpan4/kubecove" \
      org.opencontainers.image.version="$VERSION" \
      org.opencontainers.image.revision="$REVISION" \
      org.opencontainers.image.licenses="AGPL-3.0-or-later"
COPY --from=backend --chown=65534:65534 /runtime-data/ /data/
COPY --from=backend --chmod=0755 /kubeconform /usr/local/bin/kubeconform
COPY LICENSE /usr/share/doc/kubecove/LICENSE
# Preserve the deployed volume ownership contract across runtime changes.
USER 65534:65534
ENV HOME=/data XDG_CACHE_HOME=/tmp
EXPOSE 8080
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/kubecove-server"]
CMD ["--quiet", "--listen", "127.0.0.1:8080", "--data-dir", "/data/kubecove"]

FROM runtime AS slim
COPY --from=backend-slim --chmod=0755 /kubecove-server /usr/local/bin/kubecove-server

# Keep the browser UI image as the default build target.
FROM runtime AS full
COPY --from=backend-full --chmod=0755 /kubecove-server /usr/local/bin/kubecove-server
