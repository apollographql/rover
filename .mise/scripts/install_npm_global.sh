#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
INSTALLERS_DIR="$SCRIPT_DIR/../../installers/npm"

cd "$(mktemp -d)"
echo "Created test directory"
# The choice of version here is arbitrary (we just need something we know exists) so that we can test if the
# installer works, given an existing version. This way we're not at the mercy of whether the binary that corresponds
# to the latest commit exists.
npm version --prefix="$INSTALLERS_DIR" --allow-same-version 0.23.0
echo "Temporarily patched package.json to fixed stable binary"
npm install --install-links=true -g "$INSTALLERS_DIR"
echo "Installed rover as global npm package"
echo "Checking version"
rover --version
echo "Checked version, all ok!"
