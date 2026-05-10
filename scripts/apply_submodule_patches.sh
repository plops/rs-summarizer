#!/usr/bin/env bash
set -euo pipefail

# Apply stored patches to third_party libraries after first checkout.
# Usage: ./scripts/apply_submodule_patches.sh

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

PATCH_DIR="$ROOT_DIR/patches"
PATCH_FAST_UMAP="$PATCH_DIR/fast-umap-mod.patch"

if [ ! -f "$PATCH_FAST_UMAP" ]; then
  echo "No fast-umap patch found at $PATCH_FAST_UMAP. Nothing to apply."
  exit 0
fi

if [ ! -d "$ROOT_DIR/third_party/fast-umap" ]; then
  echo "third_party/fast-umap not found in the checkout. Please ensure third_party is present."
  exit 1
fi

echo "Applying fast-umap patch to third_party/fast-umap..."

pushd third_party/fast-umap > /dev/null
# Try git apply; fallback to patch if necessary
if git apply --check "$PATCH_FAST_UMAP" 2>/dev/null; then
  git apply "$PATCH_FAST_UMAP"
  echo "Patch applied (git apply)."
else
  echo "git apply failed the check, attempting 'patch -p1'..."
  if patch -p1 < "$PATCH_FAST_UMAP"; then
    echo "Patch applied via patch -p1."
  else
    echo "Failed to apply patch. Please inspect $PATCH_FAST_UMAP and apply manually."
    popd > /dev/null
    exit 2
  fi
fi

# Optionally commit changes inside third_party if it is a git repo
if [ -d .git ] || git rev-parse --git-dir >/dev/null 2>&1; then
  if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "Changes detected in third_party/fast-umap. Committing locally in that directory."
    git add -A
    git commit -m "Apply local fast-umap patch"
    # Update superproject's tracked state (if this repo tracks the directory files)
    popd > /dev/null
    git add third_party/fast-umap
    git commit -m "Update third_party/fast-umap to patched state" || true
  else
    popd > /dev/null
  fi
else
  popd > /dev/null
fi

echo "Patch application complete."
