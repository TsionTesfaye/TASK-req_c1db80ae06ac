# syntax=docker/dockerfile:1.7
#
# TerraOps single-app image.
#
# Contract kept stable across phases:
#   * final runtime image has the backend binary at /usr/local/bin/terraops-backend
#   * final runtime image serves the SPA from /app/dist
#   * entrypoint runs scripts/dev_bootstrap.sh then `terraops-backend serve`
#   * bind :8443 with Rustls
#
# P1 completion: the real Yew SPA is built with Trunk + wasm-bindgen-cli in
# the `frontend-build` stage below and its output is copied into `/app/dist/`
# of the runtime image. The P0 scaffold shell (`crates/frontend/dist-scaffold`)
# is no longer used by the runtime image.

# ---- Stage 1: backend binary ----------------------------------------------
FROM rust:1.88-bookworm AS backend-build

ENV CARGO_TERM_COLOR=never \
    CARGO_NET_RETRY=10 \
    CARGO_HTTP_MULTIPLEXING=false \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    RUSTFLAGS="-C debuginfo=0" \
    SQLX_OFFLINE=true

WORKDIR /workspace

RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml ./
COPY crates/shared   ./crates/shared
COPY crates/backend  ./crates/backend
COPY crates/frontend ./crates/frontend

# Build only the backend crate. `cargo -p terraops-backend` does not compile
# `crates/frontend`, so no wasm toolchain is pulled here.
RUN cargo build --release -p terraops-backend

# ---- Stage 2: frontend WASM bundle ----------------------------------------
FROM rust:1.88-bookworm AS frontend-build

ENV CARGO_TERM_COLOR=never \
    CARGO_NET_RETRY=10 \
    CARGO_HTTP_MULTIPLEXING=false \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    RUSTFLAGS="-C debuginfo=0"

WORKDIR /workspace

RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && rustup target add wasm32-unknown-unknown \
 && cargo install --locked trunk --version 0.21.14 \
 && cargo install --locked wasm-bindgen-cli --version 0.2.118

COPY Cargo.toml ./
COPY crates/shared   ./crates/shared
COPY crates/backend  ./crates/backend
COPY crates/frontend ./crates/frontend

WORKDIR /workspace/crates/frontend
RUN trunk build --release --offline

# ---- Final runtime ---------------------------------------------------------
FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates openssl bash tini \
 && rm -rf /var/lib/apt/lists/* \
 && groupadd --system terraops \
 && useradd  --system --gid terraops --home /app --shell /usr/sbin/nologin terraops \
 && mkdir -p /runtime /app/dist /app/scripts \
 && chown -R terraops:terraops /runtime /app

COPY --from=backend-build  /workspace/target/release/terraops-backend /usr/local/bin/terraops-backend
COPY --from=frontend-build /workspace/crates/frontend/dist/           /app/dist/

COPY scripts/dev_bootstrap.sh  /app/scripts/dev_bootstrap.sh
COPY scripts/gen_tls_certs.sh  /app/scripts/gen_tls_certs.sh
COPY scripts/seed_demo.sh      /app/scripts/seed_demo.sh
RUN chmod +x /app/scripts/*.sh \
 && chown -R terraops:terraops /app

USER terraops

EXPOSE 8443

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["bash", "-lc", "/app/scripts/dev_bootstrap.sh && terraops-backend migrate && terraops-backend seed && exec terraops-backend serve"]
