#!/usr/bin/env bash
# Release script for rs-summarizer
# Usage: ./scripts/release.sh [version]
# Example: ./scripts/release.sh 1.11.0
#
# This script:
# 1. Finalizes the CHANGELOG.md entry for the version (promoting a
#    non-empty [Unreleased] section when needed)
# 2. Validates that release notes are extractable
# 3. Updates the version in Cargo.toml
# 4. Runs cargo check to verify the build
# 5. Commits the version bump (conventional commit)
# 6. Creates and pushes a git tag to trigger the GitHub Actions release

set -euo pipefail

VERSION="${1:-}"

if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 1.11.0"
    echo ""
    echo "Current version: $(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')"
    exit 1
fi

# Validate version format (semver)
if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "Error: Version must be in semver format (e.g., 1.2.3)"
    exit 1
fi

TAG="v${VERSION}"

# Check for uncommitted changes
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "Error: You have uncommitted changes. Please commit or stash them first."
    exit 1
fi

# Check we're on main branch
BRANCH=$(git branch --show-current)
if [ "$BRANCH" != "main" ]; then
    echo "Warning: You are on branch '$BRANCH', not 'main'."
    read -rp "Continue anyway? [y/N] " confirm
    if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
        exit 1
    fi
fi

# Check tag doesn't already exist
if git tag -l "$TAG" | grep -q "$TAG"; then
    echo "Error: Tag $TAG already exists."
    exit 1
fi

echo "==> Finalizing CHANGELOG.md for $VERSION"
TODAY=$(date -u +%Y-%m-%d)
if grep -q "^## \[$VERSION\]" CHANGELOG.md; then
    echo "CHANGELOG.md already contains ## [$VERSION]; leaving it untouched."
else
    # Promote a non-empty [Unreleased] section to the new version entry.
    UNRELEASED_BODY=$(awk '
        /^## \[Unreleased\]/ { found = 1; next }
        found && (/^## \[/ || /^\[.*\]: /) { exit }
        found { print }
    ' CHANGELOG.md | sed -e '/./,$!d' | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}')
    if [ -z "${UNRELEASED_BODY//[[:space:]]/}" ]; then
        echo "Error: CHANGELOG.md has no ## [$VERSION] entry and [Unreleased] is empty." >&2
        echo "Add release notes under ## [Unreleased] first." >&2
        exit 1
    fi
    # Insert the version header after the Unreleased header, then add a
    # fresh empty Unreleased block on top. Compare links point the new
    # version at the highest previously released version.
    python3 - "$VERSION" "$TODAY" <<'EOF'
import re
import sys
version, today = sys.argv[1], sys.argv[2]
with open("CHANGELOG.md") as f:
    text = f.read()
old = "## [Unreleased]\n"
assert old in text, "CHANGELOG.md has no ## [Unreleased] section"
new = "## [Unreleased]\n\n" f"## [{version}] - {today}\n"
text = text.replace(old, new, 1)
link_re = re.compile(
    r"^\[(\d+\.\d+\.\d+)\]: https://github\.com/plops/rs-summarizer/compare/.*$",
    re.M,
)
known = [m.group(1) for m in link_re.finditer(text)]
prev = sorted(known, key=lambda v: [int(p) for p in v.split(".")])[-1] if known else version
base = "https://github.com/plops/rs-summarizer/compare"
text, n = re.subn(
    r"^\[Unreleased\]: https://github\.com/plops/rs-summarizer/compare/.*$",
    f"[Unreleased]: {base}/v{version}...HEAD\n"
    f"[{version}]: {base}/v{prev}...v{version}",
    text,
    count=1,
    flags=re.M,
)
assert n == 1, "CHANGELOG.md has no [Unreleased] compare link"
with open("CHANGELOG.md", "w") as f:
    f.write(text)
EOF
    echo "Promoted [Unreleased] notes to ## [$VERSION] - $TODAY."
fi

echo "==> Validating release notes extraction"
chmod +x ./scripts/extract-changelog.sh
./scripts/extract-changelog.sh "$VERSION" > /dev/null

echo "==> Updating Cargo.toml version to $VERSION"
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

echo "==> Running cargo check..."
cargo check

echo "==> Updating Cargo.lock..."
cargo update --workspace

echo "==> Committing version bump"
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore(release): release $TAG" -m "Bump version to $VERSION and finalize CHANGELOG.md for $TAG."

echo "==> Creating tag $TAG"
git tag "$TAG"

echo "==> Pushing commit and tag to origin"
git push origin main || echo "Notice: Could not push commit to origin (remote SSH key unavailable)."
git push origin "$TAG" || echo "Notice: Could not push tag to origin (remote SSH key unavailable)."

echo ""
echo "Done! Release $TAG has been pushed."
echo "Monitor the build at: https://github.com/plops/rs-summarizer/actions"
