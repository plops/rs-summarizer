#!/usr/bin/env bash
# Pre-release check script for rs-summarizer
# Run this before releasing to verify everything is in order.
#
# Usage: ./scripts/release-check.sh

set -euo pipefail

echo "==> Pre-release checks for rs-summarizer"
echo ""

CURRENT_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
echo "Current version: $CURRENT_VERSION"
echo ""

# Check for uncommitted changes
echo -n "Clean working tree: "
if git diff --quiet && git diff --cached --quiet; then
    echo "✓"
else
    echo "✗ (uncommitted changes)"
fi

# Check branch
echo -n "On main branch: "
BRANCH=$(git branch --show-current)
if [ "$BRANCH" = "main" ]; then
    echo "✓"
else
    echo "✗ (on $BRANCH)"
fi

# Check remote is up to date
echo -n "Up to date with origin: "
git fetch origin --quiet 2>/dev/null || true
LOCAL=$(git rev-parse HEAD)
REMOTE=$(git rev-parse origin/main 2>/dev/null || echo "unknown")
if [ "$LOCAL" = "$REMOTE" ]; then
    echo "✓"
elif [ "$REMOTE" = "unknown" ]; then
    echo "? (could not fetch origin)"
else
    echo "✗ (local and remote differ)"
fi

# Build check
echo -n "Cargo check: "
if cargo check 2>/dev/null; then
    echo "✓"
else
    echo "✗"
fi

# Clippy
echo -n "Clippy: "
if cargo clippy -- -W clippy::all 2>/dev/null; then
    echo "✓"
else
    echo "✗ (warnings or errors)"
fi

# Tests
echo -n "Unit tests: "
if cargo test 2>/dev/null; then
    echo "✓"
else
    echo "✗"
fi

echo ""
echo "If all checks pass, run:"
echo "  ./scripts/release.sh <new-version>"
