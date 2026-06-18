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

PLATFORM_PKG_NAME="@apollo/$(basename "$PLATFORM_PKG_DIR")"

# On Windows (Git Bash), pwd returns /d/a/... but npm needs D:/a/... format.
if command -v cygpath >/dev/null 2>&1; then
  INSTALLERS_DIR=$(cygpath -m "$INSTALLERS_DIR")
  PLATFORM_PKG_DIR=$(cygpath -m "$PLATFORM_PKG_DIR")
fi

TMPDIR=$(mktemp -d)
cd "$TMPDIR"

# Add the platform package as a direct dependency so npm actually installs it.
# npm `overrides` won't force-install an optional dep whose version doesn't
# exist on the registry — the dep is silently skipped before the override is
# ever evaluated.
cat > package.json << EOF
{
  "name": "rover-global-install-test",
  "version": "1.0.0",
  "dependencies": {
    "@apollo/rover": "file:${INSTALLERS_DIR}",
    "${PLATFORM_PKG_NAME}": "file:${PLATFORM_PKG_DIR}"
  }
}
EOF

npm install --install-links=true

# Install globally from the already-resolved packages. --omit=optional prevents
# npm from re-running optional dependency resolution on these already-resolved
# packages and hitting EBADPLATFORM on non-matching platforms.
npm install --install-links=true --omit=optional \
  -g "${TMPDIR}/node_modules/@apollo/rover" \
  "${TMPDIR}/node_modules/${PLATFORM_PKG_NAME}"

rover --version
