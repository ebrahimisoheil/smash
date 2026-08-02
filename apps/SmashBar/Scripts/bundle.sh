#!/bin/sh
# Wrap the SPM executable in a minimal .app bundle (menu-bar agent app).
set -e
cd "$(dirname "$0")/.."
swift build -c release
APP=.build/SmashBar.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp .build/release/SmashBar "$APP/Contents/MacOS/SmashBar"
cp Sources/SmashBar/Resources/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
cp -R .build/release/SmashBar_SmashBar.bundle "$APP/Contents/Resources/" 2>/dev/null || true
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key><string>io.github.ebrahimisoheil.smashbar</string>
    <key>CFBundleName</key><string>SmashBar</string>
    <key>CFBundleDisplayName</key><string>SmashBar</string>
    <key>CFBundleExecutable</key><string>SmashBar</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>1.0.0</string>
    <key>LSMinimumSystemVersion</key><string>14.0</string>
    <key>LSUIElement</key><true/>
    <key>CFBundleIconFile</key><string>AppIcon</string>
</dict>
</plist>
PLIST
codesign --force --sign - "$APP" 2>/dev/null || true
echo "bundled: $APP"

# --release-zip: produce the distributable zip the cask points at.
if [ "$1" = "--release-zip" ]; then
  VERSION=$(grep -o 'static let version = "[^"]*"' Sources/SmashBar/DesignSystem.swift | grep -o '"[^"]*"' | tr -d '"')
  ZIP=".build/SmashBar-${VERSION}.0.zip"
  rm -f "$ZIP"
  ditto -c -k --keepParent "$APP" "$ZIP"
  shasum -a 256 "$ZIP"
  echo "release zip: $ZIP  (attach to the GitHub release; put the sha256 in the cask)"
  exit 0
fi

# --install: replace /Applications/SmashBar.app with this build and relaunch.
if [ "$1" = "--install" ]; then
  pkill -x SmashBar 2>/dev/null || true
  sleep 1
  rm -rf /Applications/SmashBar.app
  cp -R "$APP" /Applications/SmashBar.app
  open /Applications/SmashBar.app
  echo "installed: /Applications/SmashBar.app (running)"
fi
