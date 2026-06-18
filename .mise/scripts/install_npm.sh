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

cd "$(mktemp -d)"
npm init -y
npm install --install-links=true "$INSTALLERS_DIR" "$PLATFORM_PKG_DIR"
./node_modules/.bin/rover --version
