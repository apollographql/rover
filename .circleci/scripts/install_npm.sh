#! /bin/bash
set -euo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd "$(mktemp -d)"
echo "Created test directory"
npm init -y
echo "Initialised new npm package"
npm install --install-links=true "$SCRIPT_DIR/../../installers/npm"
echo "Installed rover as local npm package"
cd node_modules/.bin/
echo "Checking version"
./rover --version
echo "Checked version, all ok!"