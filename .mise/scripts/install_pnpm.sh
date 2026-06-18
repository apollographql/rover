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

npm install -g pnpm@v9.3.0

PLATFORM_PKG_NAME="@apollo/$(basename "$PLATFORM_PKG_DIR")"
PLATFORM_PKG_DIRNAME="$(basename "$PLATFORM_PKG_DIR")"

TMPDIR=$(mktemp -d)
cd "$TMPDIR"

# Copy packages into the temp dir so pnpm can reference them with relative
# file: paths. pnpm doesn't reliably handle Windows absolute paths (e.g.
# file:D:/a/...) and prepends cwd to them — copying sidesteps the issue.
cp -r "$INSTALLERS_DIR" ./rover
cp -r "$PLATFORM_PKG_DIR" "./${PLATFORM_PKG_DIRNAME}"

# --shamefully-hoist flattens node_modules so require.resolve() in rover.js
# can find the platform package just as it would with npm.
cat > package.json << EOF
{
  "name": "rover-install-test",
  "version": "1.0.0",
  "dependencies": {
    "@apollo/rover": "file:rover",
    "${PLATFORM_PKG_NAME}": "file:${PLATFORM_PKG_DIRNAME}"
  }
}
EOF

pnpm install --no-frozen-lockfile --shamefully-hoist
./node_modules/.bin/rover --version
