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

npm install -g pnpm@v9.3.0

PLATFORM_PKG_NAME="@apollo/$(basename "$PLATFORM_PKG_DIR")"
TMPDIR=$(mktemp -d)
cd "$TMPDIR"

# pnpm v9 uses strict virtual-store isolation: installing @apollo/rover from a
# local path causes it to try the npm registry for its optionalDependencies
# (e.g. @apollo/rover-linux-x64@0.40.0), which doesn't exist yet.  The dep
# is silently skipped and require.resolve() returns null at runtime.
# Using pnpm.overrides in the project package.json forces pnpm to wire the
# local platform package into @apollo/rover's virtual node_modules instead.
cat > package.json << EOF
{
  "name": "rover-install-test",
  "version": "1.0.0",
  "dependencies": {
    "@apollo/rover": "file:${INSTALLERS_DIR}"
  },
  "pnpm": {
    "overrides": {
      "${PLATFORM_PKG_NAME}": "file:${PLATFORM_PKG_DIR}"
    }
  }
}
EOF

pnpm install --no-frozen-lockfile
./node_modules/.bin/rover --version
