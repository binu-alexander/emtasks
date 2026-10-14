#!/bin/bash
# Build emTasks.app, emTasks-<version>.dmg and the release emTasks-<version>.zip
# (dmg + INSTALL.txt + README.md + SPEC.MD + CHANGELOG.md + LICENSE) into the project root.
# Works offline. Usage: emtasks/packaging/package.sh
# Optional: SIGN_IDENTITY="Developer ID Application: …" to sign with a real certificate.
# Optional: NOTARY_PROFILE=<name> (with SIGN_IDENTITY) to notarize and staple the dmg, so
#           other Macs open it without the "Apple could not verify" warning. Create the
#           profile once with: xcrun notarytool store-credentials <name>
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
CRATE="$(dirname "$HERE")"
ROOT="$(dirname "$CRATE")"
# shellcheck disable=SC1091
source "$ROOT/activate"

VERSION="$(grep -m1 '^version' "$CRATE/Cargo.toml" | cut -d'"' -f2)"
APP="$ROOT/emTasks.app"
DMG="$ROOT/emTasks-$VERSION.dmg"
ZIP="$ROOT/emTasks-$VERSION.zip"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

if [ -n "${NOTARY_PROFILE:-}" ] && [ -z "${SIGN_IDENTITY:-}" ]; then
  echo "NOTARY_PROFILE needs SIGN_IDENTITY (a Developer ID certificate)" >&2
  exit 1
fi

echo "==> Running tests"
(cd "$CRATE" && cargo test --release --offline --locked --quiet)

echo "==> Building release binary (v$VERSION)"
(cd "$CRATE" && cargo build --release --offline --locked)

echo "==> Making icon"
[ -f "$HERE/icon-1024.png" ] || python3 -I "$HERE/make_icon.py" "$HERE/icon-1024.png"
ICONSET="$WORK/AppIcon.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s "$HERE/icon-1024.png" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z $d $d "$HERE/icon-1024.png" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$WORK/AppIcon.icns"

echo "==> Assembling emTasks.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$CRATE/target/release/emtasks" "$APP/Contents/MacOS/emtasks"
cp "$WORK/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>emTasks</string>
    <key>CFBundleDisplayName</key><string>emTasks</string>
    <key>CFBundleIdentifier</key><string>io.github.binu-alexander.emtasks</string>
    <key>CFBundleExecutable</key><string>emtasks</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>emTasks — works offline, your data stays on this Mac.</string>
</dict>
</plist>
PLIST

echo "==> Signing"
if [ -n "${SIGN_IDENTITY:-}" ]; then
  codesign --force --options runtime --timestamp --sign "$SIGN_IDENTITY" "$APP"
else
  # Ad-hoc signature with the hardened runtime (blocks code injection into the app).
  # Other Macs must still approve it once, since it isn't notarized (see README).
  codesign --force --options runtime --sign - "$APP"
fi
codesign --verify --strict "$APP"

echo "==> Creating $(basename "$DMG")"
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -quiet -volname "emTasks $VERSION" -srcfolder "$STAGE" -fs HFS+ -format UDZO "$DMG"
if [ -n "${SIGN_IDENTITY:-}" ]; then
  codesign --force --timestamp --sign "$SIGN_IDENTITY" "$DMG"
fi
if [ -n "${NOTARY_PROFILE:-}" ]; then
  echo "==> Notarizing (needs internet; usually a few minutes)"
  xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY_PROFILE" --wait
  xcrun stapler staple "$DMG"
  spctl --assess --type open --context context:primary-signature -v "$DMG"
fi

echo "==> Creating $(basename "$ZIP")"
REL="$WORK/release/emTasks-$VERSION"
mkdir -p "$REL"
cp "$DMG" "$ROOT/README.md" "$ROOT/SPEC.MD" "$ROOT/CHANGELOG.md" "$ROOT/LICENSE" "$REL/"
sed "s/{{VERSION}}/$VERSION/g" "$HERE/INSTALL.txt" > "$REL/INSTALL.txt"
rm -f "$ZIP"
(cd "$WORK/release" && ditto -c -k --norsrc --noextattr --keepParent "emTasks-$VERSION" "$ZIP")

echo
echo "Done:"
echo "  $APP"
echo "  $DMG ($(du -h "$DMG" | cut -f1))"
echo "  $ZIP ($(du -h "$ZIP" | cut -f1))"
# Fingerprint file, so recipients can check the download wasn't altered.
(cd "$ROOT" && shasum -a 256 "$(basename "$ZIP")" > "$(basename "$ZIP").sha256")
echo "  $ZIP.sha256"
echo "  SHA-256 of zip: $(cut -d' ' -f1 "$ZIP.sha256")"
