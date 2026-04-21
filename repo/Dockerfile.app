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
# Build stages
# ────────────
#   chef          — rust toolchain + cargo-chef; changes only on base image bump
#   planner       — produces recipe.json from workspace manifests (no src copy)
#   backend-build — cooks native deps (cached) → compiles backend binary
#   frontend-build— cooks WASM deps (cached) → compiles Yew SPA via trunk
#   runtime       — minimal debian image with binary + dist
#
# Incremental rebuild cost (Docker layer cache):
#   src change only          → dep-cook layer is a cache hit; only app code recompiles  (~3–5 min)
#   Cargo.toml / lock change → dep-cook layer rebuilds; full dep compile expected        (~25 min)
#   toolchain / apt change   → only on explicit version bump in this file               (~25 min)
#
# Memory note: CARGO_BUILD_JOBS=4 caps parallel rustc processes at 4 to avoid
# OOM-killing on Docker Desktop hosts limited to 7–8 GiB.  Each release-mode
# rustc job can peak at 600 MB–1 GB; 12 uncapped jobs × 1 GB = 12 GB > limit.

# ── Stage 0: cargo-chef base ─────────────────────────────────────────────────
FROM rust:1.88-bookworm AS chef

RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates curl xz-utils \
 && rm -rf /var/lib/apt/lists/* \
 && ARCH=$(uname -m) \
 && curl --connect-timeout 30 --max-time 120 -fsSL \
      "https://github.com/LukeMathWalker/cargo-chef/releases/download/v0.1.77/cargo-chef-${ARCH}-unknown-linux-gnu.tar.xz" \
    | tar -xJ --strip-components=1 -C /usr/local/bin \
         "cargo-chef-${ARCH}-unknown-linux-gnu/cargo-chef" \
 && chmod +x /usr/local/bin/cargo-chef

WORKDIR /workspace

# ── Stage 0b: workspace planner ──────────────────────────────────────────────
FROM chef AS planner

COPY Cargo.toml ./
COPY crates/shared   ./crates/shared
COPY crates/backend  ./crates/backend
COPY crates/frontend ./crates/frontend
RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 1: backend binary ───────────────────────────────────────────────────
FROM chef AS backend-build

ENV CARGO_TERM_COLOR=never \
    CARGO_NET_RETRY=10 \
    CARGO_HTTP_MULTIPLEXING=false \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    CARGO_HTTP_TIMEOUT=60 \
    CARGO_BUILD_JOBS=4 \
    RUSTFLAGS="-C debuginfo=0" \
    SQLX_OFFLINE=true

# Cook all native-target dependencies.  Cached as long as recipe.json unchanged.
COPY --from=planner /workspace/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build application code — only this layer re-runs on src changes.
COPY Cargo.toml ./
COPY crates/shared   ./crates/shared
COPY crates/backend  ./crates/backend
COPY crates/frontend ./crates/frontend
RUN cargo build --release -p terraops-backend

# ── Stage 2: frontend WASM bundle ────────────────────────────────────────────
FROM chef AS frontend-build

ENV CARGO_TERM_COLOR=never \
    CARGO_NET_RETRY=10 \
    CARGO_HTTP_MULTIPLEXING=false \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    CARGO_HTTP_TIMEOUT=60 \
    CARGO_BUILD_JOBS=4 \
    RUSTFLAGS="-C debuginfo=0"

# Install trunk and wasm-bindgen-cli from pre-built binaries.
# Trunk.toml pins wasm_bindgen = "0.2.118" and we provide the matching binary
# on PATH so trunk uses it directly without re-downloading.
#
# trunk: download to a temp file so we capture the curl exit code independently
# of tar.  If the pre-built binary is absent for this architecture (the
# v0.21.14 release omitted the x86_64 GNU asset; aarch64 was present), fall
# back to `cargo install --locked trunk` — slower (~10 min) but always works.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/* \
 && rustup target add wasm32-unknown-unknown \
 && ARCH=$(uname -m) \
 && TRUNK_TGZ="/tmp/trunk.tar.gz" \
 && TRUNK_URL="https://github.com/trunk-rs/trunk/releases/download/v0.21.14/trunk-${ARCH}-unknown-linux-gnu.tar.gz" \
 && (curl --connect-timeout 30 --max-time 120 -fsSLo "${TRUNK_TGZ}" "${TRUNK_URL}" \
     && tar -xzf "${TRUNK_TGZ}" -C /usr/local/bin trunk \
     && rm -f "${TRUNK_TGZ}" \
     && echo "trunk: pre-built binary installed for ${ARCH}") \
    || (echo "trunk: pre-built not available for ${ARCH} at v0.21.14 — falling back to cargo install" \
        && rm -f "${TRUNK_TGZ}" \
        && cargo install --locked trunk --version 0.21.14) \
 && curl --connect-timeout 30 --max-time 120 -fsSL \
      "https://github.com/rustwasm/wasm-bindgen/releases/download/0.2.118/wasm-bindgen-0.2.118-${ARCH}-unknown-linux-gnu.tar.gz" \
    | tar -xz --strip-components=1 -C /usr/local/bin \
         "wasm-bindgen-0.2.118-${ARCH}-unknown-linux-gnu/wasm-bindgen" \
 && chmod +x /usr/local/bin/trunk /usr/local/bin/wasm-bindgen

# NOTE: cargo-chef WASM cook is intentionally omitted.
# The shared recipe.json includes backend-only crates (ring, sqlx, actix-web)
# that do not compile for wasm32-unknown-unknown.  Cooking the full recipe for
# that target fails on ring (needs clang) and on other non-WASM-portable crates.
# A frontend-specific recipe from a separate workspace manifest would fix this
# but is deferred.  The main incremental benefit is still preserved via Docker
# layer caching at the COPY boundary: backend source changes do NOT invalidate
# the trunk build layer because crates/backend is not copied into this stage.

# Build the Yew SPA.
# The workspace Cargo.toml lists crates/backend as a member, so cargo metadata
# (called by trunk) requires backend/Cargo.toml to exist.  We copy only the
# manifest (not src/) so backend source changes do NOT invalidate this layer.
COPY Cargo.toml Cargo.lock ./
COPY crates/backend/Cargo.toml  ./crates/backend/Cargo.toml
COPY crates/shared   ./crates/shared
COPY crates/frontend ./crates/frontend

WORKDIR /workspace/crates/frontend
RUN trunk build --release --offline

# ── Final runtime ─────────────────────────────────────────────────────────────
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
