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
    # Update Cargo.toml (only the first match under [workspace.package])
    awk -v v="$NEW_VERSION" '/^version = / && !x {print "version = \""v"\""; x=1; next} 1' Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
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
    # Update Cargo.toml (only the first match)
    awk -v v="$NEW_VERSION" '/^version = / && !x {print "version = \""v"\""; x=1; next} 1' Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
    VERSION="v${NEW_VERSION}"
    echo "Updated Cargo.toml version to: $NEW_VERSION"
    echo "Using version: $VERSION"
fi

echo "Committing version change..."
git add Cargo.toml
git commit -m "chore(release): update version to $NEW_VERSION"


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

echo "Pushing changes and tag $VERSION to origin and gitea..."
for remote in origin gitea; do
    echo "Pushing to $remote..."
    git push $remote release --force || echo "Warning: failed to push to $remote (might be a read-only mirror)"
    git push $remote "$VERSION" --force --tags || echo "Warning: failed to push tags to $remote"
done

echo ""
echo "Calculating SHA256 for $VERSION archive..."
SHA256=$(git archive --format=tar.gz --prefix=atuin/ "$VERSION" | shasum -a 256 | awk '{print $1}')
echo "SHA256: $SHA256"

# Check if homebrew-tap exists locally
TAP_DIR="../gauravahuja.me/homebrew-tap"
FORMULA_FILE="$TAP_DIR/Formula/atuin.rb"

# Fallback path if cloned differently
if [ ! -f "$FORMULA_FILE" ]; then
    TAP_DIR="../homebrew-tap"
    FORMULA_FILE="$TAP_DIR/Formula/atuin.rb"
fi

if [ -f "$FORMULA_FILE" ]; then
    echo ""
    echo "Updating Homebrew Tap formula at $FORMULA_FILE..."
    RAW_VERSION="${VERSION#v}"

    if grep -q 'version "' "$FORMULA_FILE"; then
        "${SED_INPLACE[@]}" "s|version \".*\"|version \"${RAW_VERSION}\"|" "$FORMULA_FILE"
    else
        "${SED_INPLACE[@]}" "s|url \".*\"|url \"https://gitea.gauravahuja.dev/gauravahujame/atuin/archive/v${RAW_VERSION}.tar.gz\"|" "$FORMULA_FILE"
    fi
    "${SED_INPLACE[@]}" "s|sha256 \".*\"|sha256 \"${SHA256}\"|" "$FORMULA_FILE"

    echo "Committing and pushing Homebrew Tap..."
    (cd "$TAP_DIR" && \
     git add Formula/atuin.rb && \
     git commit -m "atuin: update to $VERSION" && \
     for remote in $(git remote); do git push $remote main; done)

    echo "Homebrew Tap updated successfully!"
else
    echo "Warning: Local homebrew-tap not found at $TAP_DIR"
    echo "Please update your formula manually with:"
    echo "  version \"${VERSION#v}\""
    echo "  sha256 \"$SHA256\""
fi

echo ""
echo "Done! The GitHub Action will now: "
echo "1. Build the release (release.yml)"
echo "2. Sync to Gitea (sync-release.yml)"
