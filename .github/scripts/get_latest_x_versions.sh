#!/opt/homebrew/bin/bash
set -euo pipefail

VERSION_COUNT=$(($1))
GITHUB_ORG=$2
GITHUB_REPO=$3
COMPONENT=$4
VERSION_TAG=$5
MAJOR_VERSION=$6

>&2 echo "VERSION_COUNT is $VERSION_COUNT"
>&2 echo "GITHUB_ORG is $GITHUB_ORG"
>&2 echo "GITHUB_REPO is $GITHUB_REPO"
>&2 echo "COMPONENT is $COMPONENT"
>&2 echo "VERSION_TAG is $VERSION_TAG"
>&2 echo "MAJOR_VERSION is $MAJOR_VERSION"

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
LATEST_PLUGIN_VERSIONS_PATH="$SCRIPT_DIR/../../latest_plugin_versions.json"
MAX_VERSION=$(jq -r --arg component "$COMPONENT" --arg version_tag "$VERSION_TAG" '.[$component].versions.[$version_tag] | sub("v";"")' < "$LATEST_PLUGIN_VERSIONS_PATH")
>&2 echo "Max Version is: $MAX_VERSION"

MAX_VERSION_FOUND=0
declare -a FINAL_VERSIONS=()
VERSIONS_FOUND=0

RELEASES_URL="https://api.github.com/repos/$GITHUB_ORG/$GITHUB_REPO/releases?per_page=100"
>&2 echo "Getting releases from $RELEASES_URL"
RAW_VERSIONS=$(curl -L -H "Accept: application/vnd.github+json" -H "X-GitHub-Api-Version: 2022-11-28" "$RELEASES_URL")
while read -r version; do
  >&2 echo "Scanning version $version"
  HIGHEST_VERSION=$(semver -c "$MAX_VERSION" "$version" | tail -1)
  if [[ $HIGHEST_VERSION == "$MAX_VERSION" ]]; then
      >&2 echo "Latest Version Found!"
      MAX_VERSION_FOUND=1
  fi;
  if [ $MAX_VERSION_FOUND -eq 1 ]; then
    if [ $VERSIONS_FOUND -lt $VERSION_COUNT ]; then
      CLEAN_VERSION=$(semver -c "$version")
      >&2 echo "Adding $CLEAN_VERSION to final list"
      FINAL_VERSIONS+=("$CLEAN_VERSION")
      (( VERSIONS_FOUND+=1 ))
    else
      break
    fi
  fi;
done < <(echo "$RAW_VERSIONS" | jq -rc --arg major_version "v$MAJOR_VERSION" '[.[] | select((.name | contains("-") | not ) and (.name | contains($major_version)))] | .[].name  | sub("v";"")')

jq -c -n '$ARGS.positional' --args -- "${FINAL_VERSIONS[@]}"