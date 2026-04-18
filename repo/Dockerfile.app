# syntax=docker/dockerfile:1.7
#
# TerraOps single-app image — SCAFFOLD (P0) variant.
#
# Contract kept stable across phases:
#   * final runtime image has the backend binary at /usr/local/bin/terraops-backend
#   * final runtime image serves the SPA from /app/dist
#   * entrypoint runs scripts/dev_bootstrap.sh then `terraops-backend serve`
#   * bind :8443 with Rustls
#
# Scaffold defers what it does not yet need (safely, per the owner’s Turn
# 0007 directive):
#   * NO `trunk build` stage: we copy `crates/frontend/dist-scaffold/` into
#     `/app/dist`. The Yew SPA source stays in `crates/frontend/` and its
#     real Trunk build is activated in P1 alongside the first WASM tests.
#   * NO `cargo install` of heavy side-tools (trunk, wasm-bindgen-cli):
#     those are P1+ concerns and will live in `Dockerfile.tests` once first
#     used.
#
# The only build-time network traffic is therefore:
#   * apt for the base OS packages
#   * cargo registry fetches for the backend crate's direct/transitive deps
#   * (no rustup toolchain downloads — the `rust:1.88-*` base bundles them)
#
# Cargo retry envs (`CARGO_NET_RETRY=10`, sparse registry) make cargo
# fetches resilient under flaky networks.

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
# `crates/frontend`, so no wasm toolchain is pulled.
RUN cargo build --release -p terraops-backend

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

COPY --from=backend-build /workspace/target/release/terraops-backend /usr/local/bin/terraops-backend

# SPA shell (scaffold): P1 replaces this COPY with a Trunk build stage.
COPY crates/frontend/dist-scaffold/ /app/dist/

COPY scripts/dev_bootstrap.sh  /app/scripts/dev_bootstrap.sh
COPY scripts/gen_tls_certs.sh  /app/scripts/gen_tls_certs.sh
COPY scripts/seed_demo.sh      /app/scripts/seed_demo.sh
RUN chmod +x /app/scripts/*.sh \
 && chown -R terraops:terraops /app

USER terraops

EXPOSE 8443

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["bash", "-lc", "/app/scripts/dev_bootstrap.sh && exec terraops-backend serve"]
