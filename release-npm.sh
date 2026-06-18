#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?Usage: $0 <version>}"
BASE_URL="https://rover.apollo.dev/tar/rover"
TARGETS=(
	"x86_64-pc-windows-msvc"
	"x86_64-unknown-linux-gnu"
	"aarch64-unknown-linux-gnu"
	"x86_64-unknown-linux-musl"
	"x86_64-apple-darwin"
	"aarch64-apple-darwin"
)

# clean

rm -r target || true
rm -r npm || true

for target in "${TARGETS[@]}"; do
	url="${BASE_URL}/${target}/v${VERSION}"
	out_dir="target/${target}/release"
	mkdir -p "${out_dir}"
	echo "Downloading ${target}..."
	curl -fSL --progress-bar "${url}" | tar -xz --strip-components=1 -C "${out_dir}"
done

cargo npm generate -p rover
# TODO: set up a shim to staged-publish instead.
# cargo npm publish -p rover --access public
