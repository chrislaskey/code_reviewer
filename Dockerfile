# The web server's image, `app_web` and the Svelte pages it serves, for
# Fly.io (fly.toml) or any container host. Build from the repository root:
#
#   docker build -t starter-pack .
#
# The Rust and Node versions follow .tool-versions.

# The Svelte pages: svelte/dist and svelte/dist-ops.
FROM node:22.22.3-trixie-slim AS pages
WORKDIR /app/svelte
RUN corepack enable
COPY svelte/package.json svelte/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY svelte/ ./
RUN pnpm build

# The web server. The whole workspace is copied since Cargo reads every
# member's manifest, but only app_web is built. The cache mounts keep the
# crates and target/ between builds on the same builder.
FROM rust:1.99-trixie AS server
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked -p app_web && \
    cp target/release/app_web /usr/local/bin/app_web

FROM debian:trixie-slim
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server /usr/local/bin/app_web /usr/local/bin/app_web
COPY --from=pages /app/svelte/dist ./svelte/dist
COPY --from=pages /app/svelte/dist-ops ./svelte/dist-ops
ENV HOST=0.0.0.0 \
    PORT=8080 \
    SVELTE_DIST=/app/svelte/dist \
    SVELTE_OPS_DIST=/app/svelte/dist-ops
EXPOSE 8080
CMD ["app_web"]
