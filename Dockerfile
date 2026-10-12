# syntax=docker/dockerfile:1
# The game server and the browser client in one small image. `fly deploy` builds this on Fly's
# own machines (nothing heavy runs on yours); see docs/DEPLOY.md.

# --- Build: the server, and the client compiled to WebAssembly. -------------------------------
# Rust pinned to the version the code is developed and tested with.
FROM rust:1.98.1-slim-bookworm AS build
RUN rustup target add wasm32-unknown-unknown \
 && apt-get update && apt-get install -y --no-install-recommends brotli && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
# The cache mounts keep downloaded crates and compiled dependencies between builds on the same
# builder, so a deploy after a small change takes a minute or two instead of several.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p go-box-server \
 && cargo build --release --locked -p go-box-client --target wasm32-unknown-unknown \
 && mkdir -p /out/web \
 && cp target/release/go-box-server /out/ \
 && cp target/wasm32-unknown-unknown/release/go-box-client.wasm web/index.html web/*.js /out/web/
# Pre-compressed copies: the server sends these to browsers that accept them (~1/3 the size).
RUN cd /out/web && for f in *; do gzip -9 -k "$f" && brotli -q 11 -k "$f"; done

# --- Run: just the server binary and the web files, as an unprivileged user. ------------------
FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --no-create-home gobox
COPY --from=build /out/go-box-server /usr/local/bin/go-box-server
COPY --from=build /out/web /app/web
# Inside the container the server listens on all interfaces; Fly's proxy is the only way in.
# Everything else cloud-specific is in fly.toml.
ENV GO_BOX_BIND=0.0.0.0:8080 GO_BOX_WEB=/app/web
USER gobox
EXPOSE 8080
CMD ["go-box-server"]
