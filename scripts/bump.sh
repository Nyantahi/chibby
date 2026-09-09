#!/usr/bin/env bash
# Bump the project version across all four version sources in one shot:
#   package.json, package-lock.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json
# Keeps Cargo.lock in sync too. Does NOT commit or tag (review first).
#
# Usage:
#   scripts/bump.sh 0.3.2          # set explicit version
#   scripts/bump.sh --commit 0.3.2 # also create the bump commit + tag v0.3.2
#
# The release workflow verifies the pushed tag matches these files, so run this
# before tagging.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

commit=0
if [ "${1:-}" = "--commit" ]; then
  commit=1
  shift
fi

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
  echo "usage: scripts/bump.sh [--commit] <version>   (e.g. 0.3.2)" >&2
  exit 2
fi

# Validate semver (major.minor.patch, optional -prerelease).
if ! printf '%s' "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'; then
  echo "error: '$VERSION' is not a valid semver version" >&2
  exit 2
fi

echo "Bumping to $VERSION ..."

# package.json + package-lock.json (npm updates both, no git tag).
npm version "$VERSION" --no-git-tag-version --allow-same-version >/dev/null

# tauri.conf.json
tmp="$(mktemp)"
jq --arg v "$VERSION" '.version = $v' src-tauri/tauri.conf.json >"$tmp"
mv "$tmp" src-tauri/tauri.conf.json

# Cargo.toml — the only line starting with `version = ` at column 0 is the
# [package] version (deps use `name = { version = ... }`). Portable across
# BSD (macOS) and GNU sed; -i.bak works on both.
sed -i.bak -E "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" src-tauri/Cargo.toml
rm -f src-tauri/Cargo.toml.bak

# Cargo.lock (sync the local crate's version entry).
( cd src-tauri && cargo update -p chibby --precise "$VERSION" >/dev/null 2>&1 ) || \
  ( cd src-tauri && cargo update -p chibby >/dev/null 2>&1 ) || true

echo "Updated: package.json, package-lock.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml, src-tauri/Cargo.lock"

files=(package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock)

if [ "$commit" -eq 1 ]; then
  git add "${files[@]}"
  git commit -m "chore: bump version to $VERSION"
  git tag "v$VERSION"
  echo "Committed and tagged v$VERSION. Push with: git push && git push origin v$VERSION"
else
  echo
  echo "Next steps:"
  echo "  git add ${files[*]}"
  echo "  git commit -m \"chore: bump version to $VERSION\""
  echo "  git tag v$VERSION && git push && git push origin v$VERSION"
fi
