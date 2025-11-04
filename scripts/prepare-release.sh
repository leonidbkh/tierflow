#!/bin/bash
# Script to prepare a new release
# Usage: ./scripts/prepare-release.sh 0.1.1

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 VERSION"
    echo "Example: $0 0.1.1"
    exit 1
fi

VERSION=$1
VERSION_TAG="v$VERSION"

echo "Preparing release $VERSION_TAG..."

# Check if we're on main branch
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "main" ]; then
    echo "Error: Must be on main branch (currently on $BRANCH)"
    exit 1
fi

# Check if working directory is clean
if [ -n "$(git status --porcelain)" ]; then
    echo "Error: Working directory is not clean"
    git status --short
    exit 1
fi

# Update version in Cargo.toml
echo "Updating Cargo.toml version to $VERSION..."
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
rm Cargo.toml.bak

# Update CHANGELOG.md
echo "Updating CHANGELOG.md..."
TODAY=$(date +%Y-%m-%d)

# Replace [Unreleased] section with version
sed -i.bak "s/## \[Unreleased\]/## [Unreleased]\\n\\n### TBD\\n\\n## [$VERSION] - $TODAY/" CHANGELOG.md

# Update version comparison link
sed -i.bak "s|\[Unreleased\]:.*|[Unreleased]: https://github.com/leonidbkh/tierflow/compare/v$VERSION...HEAD\\n[$VERSION]: https://github.com/leonidbkh/tierflow/releases/tag/v$VERSION|" CHANGELOG.md

rm CHANGELOG.md.bak

# Run tests
echo "Running tests..."
cargo test

# Build release
echo "Building release..."
cargo build --release

# Verify binary
echo "Verifying binary..."
./target/release/tierflow --version

# Create git commit
echo "Creating git commit..."
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "Release v$VERSION"

# Create git tag
echo "Creating git tag $VERSION_TAG..."
git tag -a "$VERSION_TAG" -m "Release $VERSION_TAG"

echo ""
echo "Release $VERSION_TAG prepared successfully!"
echo ""
echo "Next steps:"
echo "  1. Review the changes: git show"
echo "  2. Push to GitHub: git push origin main --tags"
echo "  3. GitHub Actions will automatically:"
echo "     - Build binaries for all platforms"
echo "     - Create GitHub release"
echo "     - Publish to crates.io"
echo ""
echo "After release is published:"
echo "  - Update Homebrew formula SHA256 checksums"
echo "  - Test installation: curl -sSfL https://raw.githubusercontent.com/leonidbkh/tierflow/main/install.sh | sh"
