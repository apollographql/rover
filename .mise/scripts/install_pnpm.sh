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
TMPDIR=$(mktemp -d)
cd "$TMPDIR"

# pnpm v9 uses strict virtual-store isolation: installing @apollo/rover from a
# local path causes it to silently skip its optionalDependencies (e.g.
# @apollo/rover-linux-x64@0.40.0) because the version doesn't exist on the
# registry yet. pnpm.overrides only redirects version resolution but won't
# force installation of a dep pnpm has already decided to skip.
# pnpm.packageExtensions injects the platform package directly into
# @apollo/rover's optional deps before the dependency graph is computed,
# so pnpm installs it into @apollo/rover's virtual node_modules and
# require.resolve() finds it at runtime.
cat > package.json << EOF
{
  "name": "rover-install-test",
  "version": "1.0.0",
  "dependencies": {
    "@apollo/rover": "file:${INSTALLERS_DIR}"
  },
  "pnpm": {
    "packageExtensions": {
      "@apollo/rover": {
        "optionalDependencies": {
          "${PLATFORM_PKG_NAME}": "file:${PLATFORM_PKG_DIR}"
        }
      }
    }
  }
}
EOF

pnpm install --no-frozen-lockfile
./node_modules/.bin/rover --version
