#!/usr/bin/env bash
# dev_bootstrap.sh
#
# Local-development bootstrap. Generates any missing runtime secrets and the
# TLS server cert into the `terraops-runtime` Docker volume mounted at
# `/runtime`. Run automatically on first boot by `Dockerfile.app` CMD.
#
# NOT the production secret-management path. Do not copy these values into
# production deployments; see README.md §Secret And Runtime Rules.

set -euo pipefail

RUNTIME_DIR="${RUNTIME_DIR:-/runtime}"
SECRETS_DIR="${RUNTIME_DIR}/secrets"
CERTS_DIR="${RUNTIME_DIR}/certs"
CA_DIR="${RUNTIME_DIR}/internal_ca"

mkdir -p "${SECRETS_DIR}" "${CERTS_DIR}" "${CA_DIR}"
chmod 700 "${SECRETS_DIR}" "${CA_DIR}"

gen_key() {
    local path="$1" bytes="$2"
    if [[ ! -s "${path}" ]]; then
        echo "[dev_bootstrap] generating ${path}"
        openssl rand -out "${path}" "${bytes}"
        chmod 600 "${path}"
    fi
}

gen_key "${SECRETS_DIR}/jwt.key"         32
gen_key "${SECRETS_DIR}/email_enc.key"   32
gen_key "${SECRETS_DIR}/email_hmac.key"  32
gen_key "${SECRETS_DIR}/image_hmac.key"  32

if [[ ! -s "${CERTS_DIR}/server.crt" || ! -s "${CERTS_DIR}/server.key" ]]; then
    echo "[dev_bootstrap] generating TLS server cert"
    SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
    "${SCRIPT_DIR}/gen_tls_certs.sh" "${CERTS_DIR}"
fi

if [[ ! -s "${CA_DIR}/ca.crt" || ! -s "${CA_DIR}/ca.key" ]]; then
    echo "[dev_bootstrap] generating internal CA (for mTLS client certs)"
    openssl req -x509 -newkey rsa:4096 -nodes \
        -keyout "${CA_DIR}/ca.key" \
        -out "${CA_DIR}/ca.crt" \
        -days 3650 \
        -subj "/CN=TerraOps Internal CA" \
        >/dev/null 2>&1
    chmod 600 "${CA_DIR}/ca.key"
fi

echo "[dev_bootstrap] ok — runtime state ready at ${RUNTIME_DIR}"
