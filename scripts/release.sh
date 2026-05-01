#!/usr/bin/env bash
# Release script for rs-summarizer
# Usage: ./scripts/release.sh [version]
# Example: ./scripts/release.sh 0.2.0
#
# This script:
# 1. Updates the version in Cargo.toml
# 2. Runs cargo check to verify the build
# 3. Commits the version bump
# 4. Creates and pushes a git tag to trigger the GitHub Actions release

set -euo pipefail

VERSION="${1:-}"

if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.2.0"
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

echo "==> Updating Cargo.toml version to $VERSION"
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

echo "==> Running cargo check..."
cargo check

echo "==> Updating Cargo.lock..."
cargo update --workspace

echo "==> Committing version bump"
git add Cargo.toml Cargo.lock
git commit -m "Release $TAG"

echo "==> Creating tag $TAG"
git tag "$TAG"

echo "==> Pushing commit and tag to origin"
git push origin main
git push origin "$TAG"

echo ""
echo "Done! Release $TAG has been pushed."
echo "Monitor the build at: https://github.com/plops/rs-summarizer/actions"
