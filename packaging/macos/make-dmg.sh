#!/bin/zsh
# Construit LOOM pour macOS : binaire universel (Apple Silicon + Intel), LOOM.app
# (double-clic = LOOM dans le Terminal), signés avec le Developer ID, dans un .dmg.
# Notarisation si NOTARY_PROFILE est défini (profil créé par `xcrun notarytool store-credentials`).
#
#   packaging/macos/make-dmg.sh            → dist/LOOM-<version>-macos.dmg
#   NOTARY_PROFILE=loom packaging/macos/make-dmg.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' loom-tui/Cargo.toml | head -1)
IDENTITY=${SIGN_IDENTITY:-"Developer ID Application: Munay Passaqui (37XSV8G838)"}
OUT=$ROOT/dist/macos
WORK=$OUT/work
rm -rf "$WORK" && mkdir -p "$WORK"

echo "==> binaires (arm64 + x86_64)"
for t in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build -q --release -p loom-tui --target $t
done
lipo -create -output "$WORK/loom" target/aarch64-apple-darwin/release/loom target/x86_64-apple-darwin/release/loom
lipo -info "$WORK/loom"

echo "==> icône"
ICON=$WORK/LOOM.iconset && mkdir -p "$ICON"
sed 's/width="128" height="128"/width="1024" height="1024"/' packaging/loom.svg > "$WORK/icon.svg"
sips -s format png "$WORK/icon.svg" --out "$WORK/icon1024.png" >/dev/null
for s in 16 32 128 256 512; do
  sips -z $s $s "$WORK/icon1024.png" --out "$ICON/icon_${s}x${s}.png" >/dev/null
  sips -z $((s*2)) $((s*2)) "$WORK/icon1024.png" --out "$ICON/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICON" -o "$WORK/LOOM.icns"

echo "==> LOOM.app"
APP=$WORK/LOOM.app
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$WORK/loom" "$APP/Contents/MacOS/loom"
cp "$WORK/LOOM.icns" "$APP/Contents/Resources/LOOM.icns"
cat > "$APP/Contents/MacOS/loom-launch" <<'SH'
#!/bin/zsh
# Ouvre LOOM dans le Terminal (LOOM est une app de terminal).
BIN="${0:A:h}/loom"
osascript -e "tell application \"Terminal\" to do script quoted form of \"$BIN\"" -e 'tell application "Terminal" to activate' >/dev/null
SH
chmod +x "$APP/Contents/MacOS/loom-launch"
cat > "$APP/Contents/Info.plist" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>LOOM</string>
  <key>CFBundleDisplayName</key><string>LOOM</string>
  <key>CFBundleIdentifier</key><string>com.munaypassaqui.loom</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleExecutable</key><string>loom-launch</string>
  <key>CFBundleIconFile</key><string>LOOM</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSMicrophoneUsageDescription</key><string>LOOM enregistre le micro pour les prises et la dictée.</string>
  <key>NSAppleEventsUsageDescription</key><string>LOOM ouvre le Terminal pour s'y lancer.</string>
</dict></plist>
PL

echo "==> signature ($IDENTITY)"
cat > "$WORK/entitlements.plist" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>com.apple.security.device.audio-input</key><true/>
  <key>com.apple.security.automation.apple-events</key><true/>
</dict></plist>
PL
SIGN=(codesign --force --timestamp --options runtime --entitlements "$WORK/entitlements.plist" --sign "$IDENTITY")
"${SIGN[@]}" "$WORK/loom"
"${SIGN[@]}" "$APP/Contents/MacOS/loom"
"${SIGN[@]}" "$APP/Contents/MacOS/loom-launch"
"${SIGN[@]}" "$APP"
codesign --verify --strict --deep "$APP"

echo "==> dmg"
STAGE=$WORK/dmg && mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
cat > "$STAGE/Lisez-moi.txt" <<TXT
LOOM $VERSION

1. Glisse LOOM dans Applications.
2. Double-clique LOOM : il s'ouvre dans le Terminal.

Dans un terminal, la commande « loom » :
  sudo ln -sf /Applications/LOOM.app/Contents/MacOS/loom /usr/local/bin/loom
(ou : brew install munaypassaqui/loom/loom)

F1 dans LOOM : toutes les touches.
TXT
DMG=$OUT/LOOM-$VERSION-macos.dmg
rm -f "$DMG"
hdiutil create -quiet -volname "LOOM $VERSION" -srcfolder "$STAGE" -fs HFS+ -format UDZO "$DMG"
codesign --force --timestamp --sign "$IDENTITY" "$DMG"
# la commande seule, pour Homebrew
tar -C "$WORK" -czf "$OUT/loom-$VERSION-macos-universal.tar.gz" loom

if [[ -n "${NOTARY_PROFILE:-}" ]]; then
  echo "==> notarisation"
  xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY_PROFILE" --wait
  xcrun stapler staple "$DMG"
  ditto -c -k "$WORK/loom" "$WORK/loom.zip"
  xcrun notarytool submit "$WORK/loom.zip" --keychain-profile "$NOTARY_PROFILE" --wait
fi
shasum -a 256 "$DMG" "$OUT/loom-$VERSION-macos-universal.tar.gz"
echo "==> $DMG"
