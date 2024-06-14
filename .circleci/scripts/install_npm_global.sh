#! /bin/bash
set -euo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

mkdir /test
cd /test
echo "Created test directory"
npm install --install-links=true -g "$SCRIPT_DIR/../../installers/npm"
echo "Installed rover as global npm package"
cd /usr/local/bin/
echo "Checking version"
./rover --version
echo "Checked version, all ok!"