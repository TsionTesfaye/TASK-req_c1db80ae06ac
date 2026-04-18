#!/usr/bin/env bash
# issue_device_cert.sh
#
# Admin tool: issue a client certificate from the TerraOps internal CA for
# a named device, print the SPKI SHA-256 pin, and emit a ready-to-POST JSON
# payload for `POST /api/v1/admin/device-certs`. The admin then registers
# the SPKI via the real admin UI/API so the Rustls `ClientCertVerifier` can
# pin it in subsequent handshakes.
#
# Usage:
#   scripts/issue_device_cert.sh <device-label> [out-dir]
#
# The real enrollment flow (SEC endpoints) and SPKI pin enforcement land in
# P1 per `plan.md`. This scaffold version only generates the material.

set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "usage: $0 <device-label> [out-dir]" >&2
    exit 2
fi

LABEL="$1"
OUT_DIR="${2:-./device_certs/${LABEL}}"
CA_DIR="${CA_DIR:-/runtime/internal_ca}"

if [[ ! -s "${CA_DIR}/ca.crt" || ! -s "${CA_DIR}/ca.key" ]]; then
    echo "error: internal CA not found at ${CA_DIR}. Run dev_bootstrap.sh first." >&2
    exit 1
fi

mkdir -p "${OUT_DIR}"

openssl req -newkey rsa:3072 -nodes \
    -keyout "${OUT_DIR}/${LABEL}.key" \
    -out    "${OUT_DIR}/${LABEL}.csr" \
    -subj   "/CN=${LABEL}" \
    >/dev/null 2>&1

openssl x509 -req \
    -in  "${OUT_DIR}/${LABEL}.csr" \
    -CA  "${CA_DIR}/ca.crt" \
    -CAkey "${CA_DIR}/ca.key" \
    -CAcreateserial \
    -days 365 \
    -out "${OUT_DIR}/${LABEL}.crt" \
    >/dev/null 2>&1

# Compute SPKI SHA-256 pin (base64) from the issued certificate.
SPKI_PIN=$(openssl x509 -in "${OUT_DIR}/${LABEL}.crt" -pubkey -noout \
    | openssl pkey -pubin -outform der \
    | openssl dgst -sha256 -binary \
    | openssl enc -base64)

chmod 600 "${OUT_DIR}/${LABEL}.key"

cat <<EOF
[issue_device_cert] wrote ${OUT_DIR}/${LABEL}.{crt,key,csr}
SPKI-SHA256-PIN (base64): ${SPKI_PIN}

Register this device with the admin API (example):
  POST /api/v1/admin/device-certs
  {
    "label": "${LABEL}",
    "spki_sha256_b64": "${SPKI_PIN}"
  }
EOF
