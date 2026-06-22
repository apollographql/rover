#!/usr/bin/env bash
set -euo pipefail

# ROVER_PACKAGES_BASE is passed explicitly from the workflow (github.workspace).
# On Windows runners it arrives as D:\a\... so backslashes are normalized to
# forward slashes before embedding in JSON file: specs.
if [[ -n "${ROVER_PACKAGES_BASE:-}" ]]; then
  BASE_DIR="${ROVER_PACKAGES_BASE//\\//}"
else
  SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
  BASE_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
fi

INSTALLERS_DIR="${BASE_DIR}/installers/npm/@apollo/rover"
PLATFORM_PKG_DIR="${BASE_DIR}/installers/npm/@apollo/${PLATFORM_PKG:?PLATFORM_PKG env var is required}"
PLATFORM_PKG_NAME="@apollo/${PLATFORM_PKG}"

TMPDIR=$(mktemp -d)
cd "$TMPDIR"

# Add the platform package as a direct dependency so npm actually installs it.
# npm `overrides` won't force-install an optional dep whose version doesn't
# exist on the registry — the dep is silently skipped before the override is
# ever evaluated.
cat > package.json << EOF
{
  "name": "rover-install-test",
  "version": "1.0.0",
  "dependencies": {
    "@apollo/rover": "file:${INSTALLERS_DIR}",
    "${PLATFORM_PKG_NAME}": "file:${PLATFORM_PKG_DIR}"
  }
}
EOF

npm install --install-links=true
./node_modules/.bin/rover --version
