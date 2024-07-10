#! /bin/bash
set -euo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd "$(mktemp -d)"
echo "Created test directory"
cd "$SCRIPT_DIR/../../installers/npm"
npm version --allow-same-version 0.23.0
echo "Temporarily patched package.json to fixed stable binary"
npm install --install-links=true -g
echo "Installed rover as global npm package"
cd /usr/local/bin/
echo "Checking version"
./rover --version
echo "Checked version, all ok!"