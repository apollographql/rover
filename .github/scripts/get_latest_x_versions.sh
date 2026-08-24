#!/opt/homebrew/bin/bash
set -euo pipefail

if [ "$#" -lt 2 ]; then
  >&2 echo "Usage: $0 COMPONENT VERSION_TAG [VERSION_TAG ...]"
  exit 1
fi

COMPONENT=$1
shift
VERSION_TAGS=("$@")

>&2 echo "COMPONENT is $COMPONENT"
>&2 echo "VERSION_TAGS are ${VERSION_TAGS[*]}"

# orbiter owns the `latest-*` alias -> concrete version mapping (rover no longer keeps a local
# copy) and reports its resolution via the `X-Version` header on a redirect-disabled HEAD
# response. This is the same public HTTP contract Rover itself uses at runtime to resolve
# `rover.apollo.dev/tar/<component>/<triple>/<alias>`, so we ask orbiter through it rather than
# reading a source file orbiter owns.
ORBITER_HOST="${APOLLO_ROVER_DOWNLOAD_HOST:-https://rover.apollo.dev}"
# X-Version depends only on COMPONENT + VERSION_TAG, not the target triple, so any triple that's
# reliably released for every version works here.
FALLBACK_TRIPLE="x86_64-unknown-linux-gnu"

declare -a CLEAN_VERSIONS=()

for VERSION_TAG in "${VERSION_TAGS[@]}"; do
  RESOLVED_HEADERS=$(curl -sS --fail -I "$ORBITER_HOST/tar/$COMPONENT/$FALLBACK_TRIPLE/$VERSION_TAG") || {
    >&2 echo "Unable to resolve version for component '$COMPONENT' with tag '$VERSION_TAG' from $ORBITER_HOST"
    exit 1
  }

  LATEST_VERSION=$(printf '%s' "$RESOLVED_HEADERS" | grep -i '^x-version:' | tr -d '\r' | awk '{print $2}')

  if [ -z "$LATEST_VERSION" ]; then
    >&2 echo "No X-Version header in response for component '$COMPONENT' with tag '$VERSION_TAG' from $ORBITER_HOST"
    exit 1
  fi

  CLEAN_VERSION="${LATEST_VERSION#v}"
  >&2 echo "Latest version resolved by orbiter for tag '$VERSION_TAG': $CLEAN_VERSION"
  CLEAN_VERSIONS+=("$CLEAN_VERSION")
done

jq -c -n '$ARGS.positional' --args -- "${CLEAN_VERSIONS[@]}"
