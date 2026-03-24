# Homebrew formula for Chibby CLI (standalone)
# To use: brew install chibby-cli
#
# This installs ONLY the CLI, without the GUI app.
# Useful for headless servers or users who prefer terminal-only workflows.
#
# For the full app with GUI + CLI, use: brew install --cask chibby

class ChibbyCli < Formula
  desc "Local-first CI/CD CLI for solo developers"
  homepage "https://github.com/Nyantahi/chibby"
  version "0.1.0"
  license "MIT"

  on_macos do
    url "https://github.com/Nyantahi/chibby/releases/download/v#{version}/chibby-cli-macos-universal.tar.gz"
    sha256 "PLACEHOLDER_MACOS_SHA256"
  end

  on_linux do
    on_intel do
      url "https://github.com/Nyantahi/chibby/releases/download/v#{version}/chibby-cli-linux-x86_64.tar.gz"
      sha256 "PLACEHOLDER_LINUX_X64_SHA256"
    end
  end

  def install
    bin.install "chibby-cli" => "chibby"
  end

  test do
    assert_match "chibby", shell_output("#{bin}/chibby --version")
  end
end
