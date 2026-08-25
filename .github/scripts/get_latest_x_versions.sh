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

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
LATEST_PLUGIN_VERSIONS_PATH="$SCRIPT_DIR/../../latest_plugin_versions.json"

declare -a CLEAN_VERSIONS=()

for VERSION_TAG in "${VERSION_TAGS[@]}"; do
  LATEST_VERSION=$(jq -er --arg component "$COMPONENT" --arg version_tag "$VERSION_TAG" '.[$component].versions[$version_tag]' "$LATEST_PLUGIN_VERSIONS_PATH") || {
    >&2 echo "Unable to read latest version for component '$COMPONENT' with tag '$VERSION_TAG'"
    exit 1
  }

  if [ -z "$LATEST_VERSION" ] || [ "$LATEST_VERSION" = "null" ]; then
    >&2 echo "No entry found for component '$COMPONENT' with tag '$VERSION_TAG' in $LATEST_PLUGIN_VERSIONS_PATH"
    exit 1
  fi

  CLEAN_VERSION="${LATEST_VERSION#v}"
  >&2 echo "Latest version pulled from manifest for tag '$VERSION_TAG': $CLEAN_VERSION"
  CLEAN_VERSIONS+=("$CLEAN_VERSION")
done

jq -c -n '$ARGS.positional' --args -- "${CLEAN_VERSIONS[@]}"
