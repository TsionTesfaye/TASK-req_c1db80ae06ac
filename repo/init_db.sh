#!/usr/bin/env bash
# init_db.sh — project-standard DB initialization.
#
# Applies migrations via the backend binary and then runs the demo seeder.
# Assumes `docker compose up --build` has at least built the `app` image.

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

echo "[init_db] applying migrations"
compose run --rm app terraops-backend migrate

echo "[init_db] seeding demo users and fixtures"
compose run --rm app /app/scripts/seed_demo.sh

echo "[init_db] done"
