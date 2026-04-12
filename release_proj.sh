#!/bin/bash
# Build locally and upload binary to GitHub Releases.
# Source code + binary are compiled and pushed to GitHub.

set -e

REPO_OWNER="aydintb"
REPO_NAME="par2dialog"
TARGET="x86_64-unknown-linux-gnu"
BIN_NAME="par2dialog"

# Auto-increment patch version in Cargo.toml
CARGO_FILE="Cargo.toml"
CURRENT_VERSION=$(grep '^version' "$CARGO_FILE" | head -1 | sed 's/.*"\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
NEW_PATCH=$((PATCH + 1))
NEW_VERSION="${MAJOR}.${MINOR}.${NEW_PATCH}"

echo "=== Incrementing version: ${CURRENT_VERSION} -> ${NEW_VERSION} ==="
sed -i "s/^version = \"${CURRENT_VERSION}\"/version = \"${NEW_VERSION}\"/" "$CARGO_FILE"

# Read new version for the rest of the script
VERSION="${NEW_VERSION}"
TAG="v${VERSION}"
ASSET_NAME="${BIN_NAME}-${TARGET}"

echo "=== Building ${BIN_NAME} v${VERSION} ==="
cargo build --release --target "${TARGET}"

DIST_DIR="dist"
mkdir -p "${DIST_DIR}"
cp "target/${TARGET}/release/${BIN_NAME}" "${DIST_DIR}/${ASSET_NAME}"

echo "=== Preparing checksum ==="
cd "${DIST_DIR}"
sha256sum * > checksums.txt
cd ..

echo "=== Uploading to GitHub release ${TAG} ==="
# Delete tag locally if it exists
git tag -d "${TAG}" 2>/dev/null || true

# Commit version bump and push source
git add "$CARGO_FILE"
git commit -m "Bump version to ${NEW_VERSION}" || true
git push origin main

# Create and push the tag
git tag "${TAG}"
git push origin "${TAG}"

# Create release and upload assets
gh release create "${TAG}" \
  --title "Release ${TAG}" \
  --generate-notes \
  "${DIST_DIR}/${ASSET_NAME}" \
  "${DIST_DIR}/checksums.txt"

echo "=== Done! Release ${TAG} is live ==="

