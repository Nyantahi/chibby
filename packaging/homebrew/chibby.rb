# Homebrew Cask formula for Chibby
# To use: brew install --cask chibby
#
# This is a template. Update the version, url, and sha256 for each release.
# Once a tap is set up (e.g. chibby-app/homebrew-tap), users install with:
#   brew tap chibby-app/tap
#   brew install --cask chibby

cask "chibby" do
  version "0.1.0"

  on_arm do
    url "https://github.com/chibby-app/chibby/releases/download/v#{version}/Chibby_#{version}_aarch64.dmg"
    sha256 "PLACEHOLDER_ARM64_SHA256"
  end

  on_intel do
    url "https://github.com/chibby-app/chibby/releases/download/v#{version}/Chibby_#{version}_x64.dmg"
    sha256 "PLACEHOLDER_X64_SHA256"
  end

  name "Chibby"
  desc "Local-first CI/CD and deployment tool for solo developers"
  homepage "https://github.com/chibby-app/chibby"

  livecheck do
    url :url
    strategy :github_latest
  end

  # Install the GUI app
  app "Chibby.app"

  # Symlink the CLI to PATH (bundled inside the app)
  binary "#{appdir}/Chibby.app/Contents/MacOS/chibby-cli", target: "chibby"

  zap trash: [
    "~/Library/Application Support/Chibby",
    "~/Library/Caches/Chibby",
    "~/Library/Preferences/com.chibby.dev.plist",
  ]
end
