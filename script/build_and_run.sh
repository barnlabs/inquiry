#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-run}"
APP_NAME="Inquiry"
BUNDLE_ID="net.barnlabs.inquiry"
MIN_SYSTEM_VERSION="14.0"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
APP_BUNDLE="$DIST_DIR/$APP_NAME.app"
APP_CONTENTS="$APP_BUNDLE/Contents"
APP_MACOS="$APP_CONTENTS/MacOS"
APP_RESOURCES="$APP_CONTENTS/Resources"
APP_BINARY="$APP_MACOS/$APP_NAME"
INFO_PLIST="$APP_CONTENTS/Info.plist"
ICON_SOURCE="$ROOT_DIR/macos/Resources/Inquiry.png"

pkill -x "$APP_NAME" >/dev/null 2>&1 || true

cd "$ROOT_DIR"
test -s THIRD_PARTY_LICENSES.html || { echo "run ./script/generate_licenses.sh first" >&2; exit 1; }
test -s "$ICON_SOURCE" || { echo "missing macOS app icon: $ICON_SOURCE" >&2; exit 1; }
cargo build
swift build
SWIFT_BINARY="$(swift build --show-bin-path)/$APP_NAME"
RUST_BINARY="$ROOT_DIR/target/debug/inquiry"

rm -rf "$APP_BUNDLE"
mkdir -p "$APP_MACOS" "$APP_RESOURCES"
cp "$SWIFT_BINARY" "$APP_BINARY"
cp "$RUST_BINARY" "$APP_RESOURCES/inquiry"
cp "$ICON_SOURCE" "$APP_RESOURCES/Inquiry.png"
cp LICENSE NOTICE THIRD_PARTY_LICENSES.html "$APP_RESOURCES/"
chmod +x "$APP_BINARY" "$APP_RESOURCES/inquiry"

cat >"$INFO_PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>$APP_NAME</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleName</key><string>$APP_NAME</string>
  <key>CFBundleDisplayName</key><string>BarnLabs Inquiry</string>
  <key>CFBundleIconFile</key><string>Inquiry.png</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>$MIN_SYSTEM_VERSION</string>
  <key>NSPrincipalClass</key><string>NSApplication</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST

/usr/bin/codesign --force --deep --sign - "$APP_BUNDLE"

open_app() { /usr/bin/open -n "$APP_BUNDLE"; }

case "$MODE" in
  run) open_app ;;
  --debug|debug) lldb -- "$APP_BINARY" ;;
  --logs|logs) open_app; /usr/bin/log stream --info --style compact --predicate "process == \"$APP_NAME\"" ;;
  --telemetry|telemetry) open_app; /usr/bin/log stream --info --style compact --predicate "subsystem == \"$BUNDLE_ID\"" ;;
  --verify|verify) open_app; sleep 1; pgrep -x "$APP_NAME" >/dev/null ;;
  *) echo "usage: $0 [run|--debug|--logs|--telemetry|--verify]" >&2; exit 2 ;;
esac
