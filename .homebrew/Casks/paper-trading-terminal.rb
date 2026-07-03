# Copy to https://github.com/tsui66/homebrew-tap/Casks/paper-trading-terminal.rb
# Users install with: brew install --cask tsui66/tap/paper-trading-terminal
#
# Update version, urls, and sha256 values after each GitHub Release.

cask "paper-trading-terminal" do
  version "0.1.0"

  on_arm do
    url "https://github.com/tsui66/paper-trading-terminal/releases/download/v#{version}/paper-trading-terminal-darwin-arm64.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end

  on_intel do
    url "https://github.com/tsui66/paper-trading-terminal/releases/download/v#{version}/paper-trading-terminal-darwin-amd64.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end

  desc "AI-native CLI for local US stock paper trading"
  homepage "https://github.com/tsui66/paper-trading-terminal"

  binary "paper"

  caveats <<~EOS
    Get started:
      paper -h
      paper tui
  EOS
end