#!/bin/bash
# Bump version, commit, tag, and push.
# GitHub Actions (.github/workflows/release.yml) handles the actual
# build + release when the tag is pushed.

set -e

BIN_NAME="par2dialog"

# Auto-increment patch version in Cargo.toml
CARGO_FILE="Cargo.toml"
CURRENT_VERSION=$(grep '^version' "$CARGO_FILE" | head -1 | sed 's/.*"\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
NEW_PATCH=$((PATCH + 1))
NEW_VERSION="${MAJOR}.${MINOR}.${NEW_PATCH}"

echo "=== Incrementing version: ${CURRENT_VERSION} -> ${NEW_VERSION} ==="
sed -i "s/^version = \"${CURRENT_VERSION}\"/version = \"${NEW_VERSION}\"/" "$CARGO_FILE"

# Commit and push the version bump
git add "$CARGO_FILE"
git commit -m "Bump version to ${NEW_VERSION}"
git push origin main

# Create and push the tag — triggers GitHub Actions release workflow
TAG="v${NEW_VERSION}"
git tag -d "${TAG}" 2>/dev/null || true
git tag "${TAG}"
git push origin "${TAG}"

echo "=== Done! Tag ${TAG} pushed — GitHub Actions will build and release ==="
