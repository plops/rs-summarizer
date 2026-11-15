#!/usr/bin/env bash
# Extract the release-notes section for one version from CHANGELOG.md.
# Usage: ./scripts/extract-changelog.sh <version>
# Example: ./scripts/extract-changelog.sh v1.11.0
# Prints the section body (without the header line) to stdout.
set -euo pipefail

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version> (e.g. $0 v1.11.0)" >&2
    exit 1
fi
# Strip a leading 'v' so both "1.11.0" and "v1.11.0" work.
VERSION="${VERSION#v}"

if [ ! -f CHANGELOG.md ]; then
    echo "Error: CHANGELOG.md not found in $(pwd)" >&2
    exit 1
fi

# Print everything between "## [<VERSION>]" and the next "## [" or link
# definition line ("[x]: ..."), then drop the header line itself.
NOTES="$(awk -v ver="$VERSION" '
    $0 == "## [" ver "]" || $0 ~ "^## \\[" ver "\\] " { found = 1; next }
    found && (/^## \[/ || /^\[.*\]: /) { exit }
    found { print }
' CHANGELOG.md)"

# Trim blank lines and fail when the section is missing or empty.
TRIMMED="$(printf '%s' "$NOTES" | sed -e '/./,$!d' -e :a -e '/^\n*$/{$d;N;ba' -e '}')"
if [ -z "${TRIMMED//[[:space:]]/}" ]; then
    echo "Error: no non-empty section for version [$VERSION] in CHANGELOG.md" >&2
    exit 1
fi

printf '## What'"'"'s Changed in v%s\n\n%s\n' "$VERSION" "$NOTES"
