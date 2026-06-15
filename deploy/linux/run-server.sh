#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -x "${SCRIPT_DIR}/server" || -d "${SCRIPT_DIR}/assets" ]]; then
  DEFAULT_APP_ROOT="${SCRIPT_DIR}"
else
  DEFAULT_APP_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
fi
APP_ROOT="${APP_ROOT:-${DEFAULT_APP_ROOT}}"

SERVER_BIN="${SERVER_BIN:-${APP_ROOT}/target/release/server}"
if [[ ! -x "${SERVER_BIN}" && -x "${APP_ROOT}/server" ]]; then
  SERVER_BIN="${APP_ROOT}/server"
fi

if [[ ! -x "${SERVER_BIN}" ]]; then
  echo "server binary not found or not executable: ${SERVER_BIN}" >&2
  exit 1
fi

cd "${APP_ROOT}"
mkdir -p saves

export RUST_LOG="${RUST_LOG:-info}"
if [[ -d "${APP_ROOT}/lib" ]]; then
  export LD_LIBRARY_PATH="${APP_ROOT}/lib${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
fi

if [[ "${SERVER_BIN##*/}" != "server" ]]; then
  exec "${SERVER_BIN}" --coop-server ${SERVER_ARGS:-} "$@"
else
  exec "${SERVER_BIN}" ${SERVER_ARGS:-} "$@"
fi
