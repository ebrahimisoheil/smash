# SmashBar cask — copy into ebrahimisoheil/homebrew-Smash:Casks/smashbar.rb at
# release time, filling in the sha256 printed by `bundle.sh --release-zip`.
cask "smashbar" do
  version "1.0.0"
  sha256 "REPLACE_WITH_ZIP_SHA256"

  url "https://github.com/ebrahimisoheil/smash/releases/download/v2.0.0/SmashBar-#{version}.zip"
  name "SmashBar"
  desc "Smash's agent memory, ambient in the menu bar"
  homepage "https://github.com/ebrahimisoheil/smash"

  depends_on formula: "ebrahimisoheil/smash/Smash"
  depends_on macos: ">= :sonoma"

  app "SmashBar.app"

  # SmashBar ships unsigned (open source, no Apple Developer certificate).
  # Homebrew quarantines staged apps by default, which would block first
  # launch of an unsigned bundle; stripping the flag here restores the
  # normal double-click experience. Verified on macOS 15 and 26.
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-dr", "com.apple.quarantine", "#{appdir}/SmashBar.app"]
  end

  zap trash: []
end
