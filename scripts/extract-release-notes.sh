#!/usr/bin/env bash
# Extract the release notes for a specific version from a changelog file.
#
# The changelog format mirrors `crates/lumia-app/src/update_check.rs`:
#   - Version sections start with `## vX.Y.Z` (leading `v`/`V` optional).
#   - A `---` separator or the next `## vX.Y.Z` header ends the section.
#
# Usage: ./scripts/extract-release-notes.sh <tag> [changelog-path]
#   tag           e.g. "v0.2.0" or "0.2.0"
#   changelog-path  default "Changelog.md"
#   Prints the matched section body to stdout.
set -euo pipefail

TAG="${1:-}"
CHANGELOG="${2:-Changelog.md}"

if [ -z "$TAG" ]; then
  echo "Usage: $0 <tag> [changelog-path]" >&2
  exit 1
fi

# Strip a leading 'v' so tags and changelog headers match regardless of prefix.
VERSION="${TAG#v}"

awk -v ver="$VERSION" '
  BEGIN { found = 0 }
  /^## / {
    if (found) exit
    token = substr($0, 4)
    sub(/ .*$/, "", token)
    gsub(/^[vV]/, "", token)
    if (token == ver) { found = 1; next }
  }
  found && /^---$/ { exit }
  found { print }
' "$CHANGELOG"
