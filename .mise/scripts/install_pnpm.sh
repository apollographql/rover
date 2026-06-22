#!/usr/bin/env bash
set -euo pipefail

# Use SCRIPT_DIR for bash file ops (cp) since it's always a valid Unix path.
# pnpm package.json uses relative file: paths so no Windows path conversion needed.
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
INSTALLERS_DIR="$SCRIPT_DIR/../../installers/npm/@apollo/rover"
PLATFORM_PKG_DIR="$SCRIPT_DIR/../../installers/npm/@apollo/${PLATFORM_PKG:?PLATFORM_PKG env var is required}"

npm install -g pnpm@v9.3.0

TMPDIR=$(mktemp -d)
cd "$TMPDIR"

# Copy packages into the temp dir so pnpm can reference them with relative
# file: paths. pnpm doesn't reliably handle Windows absolute paths (e.g.
# file:D:/a/...) and prepends cwd to them — copying sidesteps the issue.
cp -r "$INSTALLERS_DIR" ./rover
cp -r "$PLATFORM_PKG_DIR" "./${PLATFORM_PKG}"

# --shamefully-hoist flattens node_modules so require.resolve() in rover.js
# can find the platform package just as it would with npm.
cat > package.json << EOF
{
  "name": "rover-install-test",
  "version": "1.0.0",
  "dependencies": {
    "@apollo/rover": "file:rover",
    "@apollo/${PLATFORM_PKG}": "file:${PLATFORM_PKG}"
  }
}
EOF

pnpm install --no-frozen-lockfile --shamefully-hoist
./node_modules/.bin/rover --version
