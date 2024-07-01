#! /bin/bash
set -euo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd "$(mktemp -d)"
echo "Created test directory"
npm install -g pnpm@v9.3.0
echo "Installed pnpm"
pnpm init
pnpm add "file:$SCRIPT_DIR/../../installers/npm"
echo "Installed rover as pnpm package"
cd node_modules/.bin/
echo "Checking version"
./rover --version
echo "Checked version, all ok!"