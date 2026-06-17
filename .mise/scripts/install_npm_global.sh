#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
INSTALLERS_DIR="$SCRIPT_DIR/../../installers/npm"
PLATFORMS_DIR="$INSTALLERS_DIR/platforms"

PLATFORM_PKG_DIR=""
for dir in "$PLATFORMS_DIR"/*/; do
  if [[ -f "${dir}bin/rover" || -f "${dir}bin/rover.exe" ]]; then
    PLATFORM_PKG_DIR="${dir%/}"
    break
  fi
done

if [[ -z "$PLATFORM_PKG_DIR" ]]; then
  echo "No built rover binary found under $PLATFORMS_DIR"
  echo "Place the rover binary in the appropriate platforms/<pkg>/bin/ directory first."
  exit 1
fi

# Artifact downloads don't preserve execute permissions on Unix.
[[ -f "${PLATFORM_PKG_DIR}/bin/rover" ]] && chmod +x "${PLATFORM_PKG_DIR}/bin/rover"

npm install --install-links=true -g "$INSTALLERS_DIR" "$PLATFORM_PKG_DIR"
rover --version
