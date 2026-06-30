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
TMPDIR=$(mktemp -d)
cd "$TMPDIR"

# Use npm overrides so the local platform package satisfies @apollo/rover's
# optionalDependency instead of npm trying (and silently failing) to fetch
# the not-yet-published version from the registry.
cat > package.json << EOF
{
  "name": "rover-global-install-test",
  "version": "1.0.0",
  "dependencies": {
    "@apollo/rover": "file:${INSTALLERS_DIR}"
  },
  "overrides": {
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
