#!/usr/bin/env bash
set -euo pipefail

# Build Lumia for macOS and assemble a distributable .app bundle and
# a drag-to-install .dmg. Mirrors scripts/build-windows-installers.ps1.
#
# Usage:
#   ./scripts/build-macos-installer.sh            Build Lumia.app + Lumia-macos-*.dmg
#   ./scripts/build-macos-installer.sh --no-dmg  Skip the .dmg step

NO_DMG=0
if [ "${1:-}" = "--no-dmg" ]; then
    NO_DMG=1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

APP_NAME="Lumia"
BUNDLE_ID="com.ifence.lumia"
TARGET_DIR="target"
APP_DIR="$TARGET_DIR/$APP_NAME.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"
PHOTOSHOP_PLUGIN_DIR="$MACOS_DIR/plugins/lumia-plugin-photoshop"
JPEG_XL_PLUGIN_DIR="$MACOS_DIR/plugins/lumia-plugin-jpeg-xl"
JPEG2000_PLUGIN_DIR="$MACOS_DIR/plugins/lumia-plugin-jpeg2000"

echo "Building release binaries..."
cargo build --release -p lumia-app -p lumia-plugin-photoshop -p lumia-plugin-jpeg-xl -p lumia-plugin-jpeg2000

echo "Assembling $APP_NAME.app..."
rm -rf "$APP_DIR"
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR" "$PHOTOSHOP_PLUGIN_DIR" "$JPEG_XL_PLUGIN_DIR" "$JPEG2000_PLUGIN_DIR"

cp "target/release/lumia-app" "$MACOS_DIR/"
cp "target/release/lumia-plugin-photoshop" "$PHOTOSHOP_PLUGIN_DIR/"
cp "plugins/lumia-plugin-photoshop/lumia.plugin.json" "$PHOTOSHOP_PLUGIN_DIR/"
cp "target/release/lumia-plugin-jpeg-xl" "$JPEG_XL_PLUGIN_DIR/"
cp "plugins/lumia-plugin-jpeg-xl/lumia.plugin.json" "$JPEG_XL_PLUGIN_DIR/"
cp "target/release/lumia-plugin-jpeg2000" "$JPEG2000_PLUGIN_DIR/"
cp "plugins/lumia-plugin-jpeg2000/lumia.plugin.json" "$JPEG2000_PLUGIN_DIR/"
chmod +x "$MACOS_DIR/lumia-app" \
    "$PHOTOSHOP_PLUGIN_DIR/lumia-plugin-photoshop" \
    "$JPEG_XL_PLUGIN_DIR/lumia-plugin-jpeg-xl" \
    "$JPEG2000_PLUGIN_DIR/lumia-plugin-jpeg2000"

# Icon: App.icns is committed in resources and referenced by Info.plist
# via CFBundleIconFile, so the bundle shows the proper Dock/Finder icon.
if [ -f "crates/lumia-app/resources/App.icns" ]; then
    cp "crates/lumia-app/resources/App.icns" "$RESOURCES_DIR/App.icns"
    echo "  ✓ Icon installed to $RESOURCES_DIR/App.icns"
else
    echo "  ! App.icns not found — bundle will use the default icon" >&2
fi

cp "crates/lumia-app/resources/Info.plist" "$CONTENTS_DIR/"

if [ "$NO_DMG" -eq 1 ]; then
    echo "Done. $APP_DIR is ready."
    exit 0
fi

echo "Creating .dmg..."
case "$(uname -m)" in
    arm64) DMG_ARCH="arm64" ;;
    x86_64) DMG_ARCH="x64" ;;
    *)
        echo "Unsupported macOS architecture: $(uname -m)" >&2
        exit 1
        ;;
esac
DMG_NAME="$APP_NAME-macos-$DMG_ARCH.dmg"
STAGING="$(mktemp -d)"
ln -s /Applications "$STAGING/Applications"
cp -R "$APP_DIR" "$STAGING/"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGING" -ov -format UDZO "$TARGET_DIR/$DMG_NAME"
rm -rf "$STAGING"

echo "Done. $APP_DIR and $TARGET_DIR/$DMG_NAME are ready."
