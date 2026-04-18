#!/usr/bin/env bash
# seed_demo.sh
#
# Seed one demo user per role plus cross-domain fixtures. At scaffold time
# the identity schema (migration 0001) exists but RBAC + auth helpers
# (argon2, email_crypto) land in P1, so this script does nothing in scaffold
# mode beyond printing the demo credentials that the README documents.
#
# The real seeding logic is wired up in P1 together with `auth/argon2.rs`
# and `auth/email_crypto.rs`. Keep this file stable; only its body grows.

set -euo pipefail

cat <<'EOF'
[seed_demo] scaffold mode — no DB writes performed.

Demo accounts (all passwords: TerraOps!2026 ; set in P1 when auth lands):
  Administrator   admin@terraops.local
  Data Steward    steward@terraops.local
  Analyst         analyst@terraops.local
  Recruiter       recruiter@terraops.local
  Regular User    user@terraops.local

Run `./init_db.sh` once auth + seeding land in P1 to materialize these
users (and the cross-domain fixtures) against the running Postgres.
EOF
