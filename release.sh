#!/bin/bash
set -e

if [ -z "$1" ]; then
    echo "Usage: ./release.sh <version>"
    echo "   or: ./release.sh auto  (detects version from Cargo.toml and adds -patch suffix)"
    exit 1
fi

VERSION=$1

# Ensure we are on release branch
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "release" ]; then
    echo "Error: You must be on the 'release' branch to release."
    echo "Current branch: $BRANCH"
    exit 1
fi

# Detect OS for sed -i compatibility
if [[ "$OSTYPE" == "darwin"* ]]; then
    SED_INPLACE=(sed -i '')
else
    SED_INPLACE=(sed -i)
fi

# Auto-detect version if requested
if [ "$VERSION" == "auto" ]; then
    # Extract version from Cargo.toml
    BASE_VERSION=$(grep '^version = ' Cargo.toml | head -n 1 | cut -d '"' -f 2)
    # Remove any existing -patch suffix before adding it back
    BASE_VERSION="${BASE_VERSION%-patch}"
    NEW_VERSION="${BASE_VERSION}-patch"
    # Update Cargo.toml with the new version
    "${SED_INPLACE[@]}" "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
    VERSION="v${NEW_VERSION}"
    echo "Updated Cargo.toml version to: $NEW_VERSION"
    echo "Detected version: $VERSION"
else
    # For manual version, assume it's provided as vX.Y.Z, add -patch
    if [[ "$VERSION" =~ ^v ]]; then
        BASE_VERSION="${VERSION#v}"
    else
        BASE_VERSION="$VERSION"
    fi
    BASE_VERSION="${BASE_VERSION%-patch}"
    NEW_VERSION="${BASE_VERSION}-patch"
    # Update Cargo.toml
    "${SED_INPLACE[@]}" "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
    VERSION="v${NEW_VERSION}"
    echo "Updated Cargo.toml version to: $NEW_VERSION"
    echo "Using version: $VERSION"
fi

echo "Syncing with upstream..."
git fetch upstream
git rebase upstream/main

# Check if tag exists
if git rev-parse "$VERSION" >/dev/null 2>&1; then
    echo "Tag $VERSION already exists locally."
    read -p "Tag already exists. Delete and recreate? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git tag -d "$VERSION"
        git tag "$VERSION"
    fi
else
    echo "Creating tag $VERSION..."
    git tag "$VERSION"
fi

echo "Pushing changes and tag $VERSION to origin (your fork)..."
git push origin release --force
git push origin "$VERSION" --force --tags

echo "Done! The GitHub Action will now: "
echo "1. Build the release (release.yml)"
echo "2. Sync to Gitea (sync-release.yml)"
echo "3. Update the Homebrew Tap (sync-release.yml)"
