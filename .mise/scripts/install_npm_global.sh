#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${ROVER_PACKAGES_BASE:-}" ]]; then
  BASE_DIR="${ROVER_PACKAGES_BASE//\\//}"
else
  SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
  BASE_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
fi

INSTALLERS_DIR="${BASE_DIR}/installers/npm/@apollo/rover"
PLATFORM_PKG_DIR="${BASE_DIR}/installers/npm/@apollo/${PLATFORM_PKG:?PLATFORM_PKG env var is required}"

# Install @apollo/rover and the platform package directly into global node_modules.
# --omit=optional prevents npm from trying to resolve platform-specific packages
# from the registry (they don't exist at the test version) for non-matching platforms.
npm install --install-links=true --omit=optional \
  -g \
  "file:${INSTALLERS_DIR}" \
  "file:${PLATFORM_PKG_DIR}"

rover --version
