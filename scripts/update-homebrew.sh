#!/bin/bash
# Update Homebrew formulas with new version and SHA256 hashes
# Usage: ./update-homebrew.sh <version> <tap_path>

set -e

VERSION=$1
TAP_PATH=${2:-../homebrew-chibby}
HASH_FILE="dist/homebrew/hashes.txt"

if [ -z "$VERSION" ]; then
  echo "Usage: $0 <version> [tap_path]"
  exit 1
fi

if [ ! -f "$HASH_FILE" ]; then
  echo "Error: $HASH_FILE not found. Run homebrew-compute-hashes first."
  exit 1
fi

echo "Updating Homebrew formulas to v$VERSION..."

# Extract hashes
SHA_ARM=$(grep 'aarch64.dmg' "$HASH_FILE" | awk '{print $1}' || echo "")
SHA_X64=$(grep -E 'x64\.dmg|x86_64\.dmg' "$HASH_FILE" | awk '{print $1}' || echo "")
SHA_CLI_MACOS=$(grep 'chibby-cli-macos' "$HASH_FILE" | awk '{print $1}' || echo "")
SHA_CLI_LINUX=$(grep 'chibby-cli-linux' "$HASH_FILE" | awk '{print $1}' || echo "")

echo "  ARM64 DMG: ${SHA_ARM:-not found}"
echo "  x64 DMG: ${SHA_X64:-not found}"
echo "  CLI macOS: ${SHA_CLI_MACOS:-not found}"
echo "  CLI Linux: ${SHA_CLI_LINUX:-not found}"

# Update Cask formula
CASK_FILE="$TAP_PATH/Casks/chibby.rb"
if [ -f "$CASK_FILE" ]; then
  echo "Updating $CASK_FILE..."
  
  # Update version
  sed -i '' 's/version ".*"/version "'"$VERSION"'"/' "$CASK_FILE"
  
  # Update ARM64 sha256 (matches sha256 "..." after on_arm block's url)
  if [ -n "$SHA_ARM" ]; then
    # Use awk to find and replace sha256 in on_arm block
    awk -v sha="$SHA_ARM" '
      /on_arm do/ { in_arm=1 }
      in_arm && /sha256/ { sub(/sha256 ".*"/, "sha256 \"" sha "\""); in_arm=0 }
      { print }
    ' "$CASK_FILE" > "$CASK_FILE.tmp" && mv "$CASK_FILE.tmp" "$CASK_FILE"
  fi
  
  # Update x64 sha256 (in on_intel block)
  if [ -n "$SHA_X64" ]; then
    awk -v sha="$SHA_X64" '
      /on_intel do/ { in_intel=1 }
      in_intel && /sha256/ { sub(/sha256 ".*"/, "sha256 \"" sha "\""); in_intel=0 }
      { print }
    ' "$CASK_FILE" > "$CASK_FILE.tmp" && mv "$CASK_FILE.tmp" "$CASK_FILE"
  fi
  
  echo "  Updated cask formula"
else
  echo "  Warning: $CASK_FILE not found, skipping"
fi

# Update CLI formula
CLI_FILE="$TAP_PATH/Formula/chibby-cli.rb"
if [ -f "$CLI_FILE" ]; then
  echo "Updating $CLI_FILE..."
  
  # Update version
  sed -i '' 's/version ".*"/version "'"$VERSION"'"/' "$CLI_FILE"
  
  # Update macOS sha256
  if [ -n "$SHA_CLI_MACOS" ]; then
    awk -v sha="$SHA_CLI_MACOS" '
      /on_macos do/ { in_macos=1 }
      in_macos && /sha256/ { sub(/sha256 ".*"/, "sha256 \"" sha "\""); in_macos=0 }
      { print }
    ' "$CLI_FILE" > "$CLI_FILE.tmp" && mv "$CLI_FILE.tmp" "$CLI_FILE"
  fi
  
  # Update Linux sha256
  if [ -n "$SHA_CLI_LINUX" ]; then
    awk -v sha="$SHA_CLI_LINUX" '
      /on_linux do/,/end/ {
        if (/sha256/) { sub(/sha256 ".*"/, "sha256 \"" sha "\"") }
      }
      { print }
    ' "$CLI_FILE" > "$CLI_FILE.tmp" && mv "$CLI_FILE.tmp" "$CLI_FILE"
  fi
  
  echo "  Updated CLI formula"
else
  echo "  Warning: $CLI_FILE not found, skipping"
fi

echo "Done! Review changes in $TAP_PATH before committing."
