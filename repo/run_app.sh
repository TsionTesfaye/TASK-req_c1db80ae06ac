#!/usr/bin/env bash
# run_app.sh — helper wrapper around `docker compose up --build`.
#
# The canonical runtime contract is `docker compose up --build`; this
# wrapper only adds a health-wait convenience on top and must not
# introduce additional host-side toolchain requirements beyond Docker.

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
cd "${REPO_ROOT}"

compose() {
    if docker compose version >/dev/null 2>&1; then
        docker compose "$@"
    else
        docker-compose "$@"
    fi
}

echo "[run_app] docker compose up --build (Ctrl-C to stop)"
compose up --build
