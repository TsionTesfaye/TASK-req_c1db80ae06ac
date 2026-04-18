#!/usr/bin/env bash
# seed_demo.sh
#
# Seed one demo user per canonical role. Idempotent.
#
# This script is the project-standard way to materialize the demo accounts
# documented in README.md. When run inside the running `app` container (the
# runtime path used by `docker compose up --build` via Dockerfile.app's CMD)
# it invokes `terraops-backend seed`, which:
#
#   * connects via $DATABASE_URL,
#   * ensures migrations are applied (caller already ran `migrate`),
#   * derives email ciphertext + hash + mask with the runtime keys,
#   * inserts or updates one user per canonical role with the README
#     credentials, and
#   * rebuilds the role grant to exactly the intended role.
#
# When run from the host (i.e. from `./init_db.sh`), it forwards into the
# running `app` service so the same code path is used.

set -euo pipefail

if command -v terraops-backend >/dev/null 2>&1; then
    # Inside the app container — call the binary directly.
    exec terraops-backend seed
fi

if command -v docker >/dev/null 2>&1 && docker compose ps --services 2>/dev/null | grep -qx app; then
    exec docker compose exec -T app terraops-backend seed
fi

cat >&2 <<'EOF'
[seed_demo] terraops-backend not on PATH and no running `app` service found.
Run `docker compose up --build` first, then `./init_db.sh` or
`docker compose exec app terraops-backend seed`.
EOF
exit 1
