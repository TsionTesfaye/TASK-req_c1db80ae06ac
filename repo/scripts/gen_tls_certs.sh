#!/usr/bin/env bash
# gen_tls_certs.sh
#
# Generate a dev-only self-signed TLS server cert for `https://localhost:8443`.
# Writes `server.crt` + `server.key` (PEM) into the target directory.

set -euo pipefail

DEST_DIR="${1:-/runtime/certs}"
mkdir -p "${DEST_DIR}"

openssl req -x509 -newkey rsa:4096 -nodes \
    -keyout "${DEST_DIR}/server.key" \
    -out    "${DEST_DIR}/server.crt" \
    -days 365 \
    -subj "/CN=localhost" \
    -addext "subjectAltName=DNS:localhost,IP:127.0.0.1,IP:0.0.0.0" \
    >/dev/null 2>&1

chmod 600 "${DEST_DIR}/server.key"
chmod 644 "${DEST_DIR}/server.crt"

echo "[gen_tls_certs] wrote ${DEST_DIR}/server.{crt,key}"
