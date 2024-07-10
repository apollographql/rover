#! /bin/bash
set -euo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd "$(mktemp -d)"
echo "Created test directory"
cd "$SCRIPT_DIR/../../installers/npm"
# The choice of version here is arbitrary (we just need something we know exists) so that we can test if the
# installer works, given an existing version. This way we're not at the mercy of whether the binary that corresponds
# to the latest commit exists.
npm version --allow-same-version 0.23.0
echo "Temporarily patched package.json to fixed stable binary"
npm install --install-links=true -g "$SCRIPT_DIR/../../installers/npm"
echo "Installed rover as global npm package"
cd /usr/local/bin/
echo "Checking version"
./rover --version
echo "Checked version, all ok!"