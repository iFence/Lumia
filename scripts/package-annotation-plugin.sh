#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

PLATFORM="${1:-$(uname -s | tr '[:upper:]' '[:lower:]')}"
ARCH="${2:-$(uname -m)}"
PACKAGE_NAME="Lumia-Annotation-$PLATFORM-$ARCH"
STAGING_DIR="target/$PACKAGE_NAME"
PLUGIN_DIR="$STAGING_DIR/lumia-plugin-annotation"
ARCHIVE="target/$PACKAGE_NAME.lumiaplugin"

cargo build --release -p lumia-plugin-annotation

rm -rf "$STAGING_DIR"
mkdir -p "$PLUGIN_DIR"
cp target/release/lumia-plugin-annotation "$PLUGIN_DIR/"
cp plugins/lumia-plugin-annotation/lumia.plugin.json "$PLUGIN_DIR/"
cp plugins/lumia-plugin-annotation/lumia.plugin.sig "$PLUGIN_DIR/"
cp -R plugins/lumia-plugin-annotation/assets "$PLUGIN_DIR/"
chmod +x "$PLUGIN_DIR/lumia-plugin-annotation"

test -x "$PLUGIN_DIR/lumia-plugin-annotation"
test -f "$PLUGIN_DIR/lumia.plugin.json"
test -f "$PLUGIN_DIR/lumia.plugin.sig"
test -f "$PLUGIN_DIR/assets/pin.svg"

APP_VERSION="$(awk -F'"' '/^version = / { print $2; exit }' crates/lumia-app/Cargo.toml)"
PLUGIN_API_VERSION="$(awk '/PROTOCOL_VERSION: u32 = / { gsub(";", "", $6); print $6; exit }' crates/lumia-plugin-api/src/rpc.rs)"
node scripts/sign-plugin-package.mjs \
  --root "$STAGING_DIR" \
  --install-directory lumia-plugin-annotation \
  --plugin-id lumia.annotation \
  --target-os "$PLATFORM" \
  --target-arch "$ARCH" \
  --minimum-lumia-version "$APP_VERSION" \
  --plugin-api-version "$PLUGIN_API_VERSION"

test -f "$STAGING_DIR/lumia.package.json"
test -f "$STAGING_DIR/lumia.package.sig"
rm -f "$ARCHIVE"
(
  cd "$STAGING_DIR"
  zip -qr "$ROOT_DIR/$ARCHIVE" .
)
echo "Created $ARCHIVE"
