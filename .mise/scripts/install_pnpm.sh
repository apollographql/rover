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

# On Windows (Git Bash), pwd returns /d/a/... but pnpm needs file:///D:/a/... format.
# (Unlike npm, pnpm treats file:D:/... as relative and prepends cwd to it.)
if command -v cygpath >/dev/null 2>&1; then
  INSTALLERS_DIR="file:///$(cygpath -m "$INSTALLERS_DIR")"
  PLATFORM_PKG_DIR="file:///$(cygpath -m "$PLATFORM_PKG_DIR")"
else
  INSTALLERS_DIR="file:${INSTALLERS_DIR}"
  PLATFORM_PKG_DIR="file:${PLATFORM_PKG_DIR}"
fi

TMPDIR=$(mktemp -d)
cd "$TMPDIR"

# Add the platform package as a direct dependency so pnpm actually installs it.
# pnpm.packageExtensions doesn't override the version already declared in
# @apollo/rover's own package.json — pnpm resolves the original "0.40.0" and
# silently skips the optional dep because that version isn't on the registry.
# --shamefully-hoist flattens node_modules so require.resolve() in rover.js
# can find the platform package just as it would with npm.
cat > package.json << EOF
{
  "name": "rover-install-test",
  "version": "1.0.0",
  "dependencies": {
    "@apollo/rover": "${INSTALLERS_DIR}",
    "${PLATFORM_PKG_NAME}": "${PLATFORM_PKG_DIR}"
  }
}
EOF

pnpm install --no-frozen-lockfile --shamefully-hoist
./node_modules/.bin/rover --version
