#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
INSTALLERS_DIR="$SCRIPT_DIR/../../installers/npm/@apollo/rover"
PLATFORMS_DIR="$SCRIPT_DIR/../../installers/npm/@apollo"

PLATFORM_PKG_DIR=""
for dir in "$PLATFORMS_DIR"/rover-*/; do
  if [[ -f "${dir}rover" || -f "${dir}rover.exe" ]]; then
    PLATFORM_PKG_DIR="${dir%/}"
    break
  fi
done

if [[ -z "$PLATFORM_PKG_DIR" ]]; then
  echo "No built rover binary found under $PLATFORMS_DIR"
  echo "Place the rover binary in the appropriate platforms/<pkg>/ directory first."
  exit 1
fi

# Artifact downloads don't preserve execute permissions on Unix.
[[ -f "${PLATFORM_PKG_DIR}/rover" ]] && chmod +x "${PLATFORM_PKG_DIR}/rover"

# On Windows (Git Bash), pwd returns /d/a/... but npm needs D:/a/... format.
if command -v cygpath >/dev/null 2>&1; then
  INSTALLERS_DIR=$(cygpath -m "$INSTALLERS_DIR")
  PLATFORM_PKG_DIR=$(cygpath -m "$PLATFORM_PKG_DIR")
fi

# Install @apollo/rover and the platform package directly into global node_modules.
# --omit=optional prevents npm from trying to resolve platform-specific packages
# from the registry (they don't exist at the test version) for non-matching platforms.
npm install --install-links=true --omit=optional \
  -g \
  "file:${INSTALLERS_DIR}" \
  "file:${PLATFORM_PKG_DIR}"

rover --version
