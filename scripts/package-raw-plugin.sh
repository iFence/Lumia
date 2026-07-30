#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 5 ]]; then
  echo "usage: $0 <platform> <arch> <bridge-library> <libraw-library> <libraw-license-directory>" >&2
  exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

PLATFORM="$1"
ARCH="$2"
BRIDGE_LIBRARY="$3"
LIBRAW_LIBRARY="$4"
LIBRAW_LICENSE_DIRECTORY="$5"
PACKAGE_NAME="Lumia-RAW-$PLATFORM-$ARCH"
STAGING_DIR="target/$PACKAGE_NAME"
PLUGIN_DIR="$STAGING_DIR/lumia-plugin-raw"
ARCHIVE="target/$PACKAGE_NAME.lumiaplugin"

for file in "$BRIDGE_LIBRARY" "$LIBRAW_LIBRARY" \
  "$LIBRAW_LICENSE_DIRECTORY/LICENSE.LGPL" "$LIBRAW_LICENSE_DIRECTORY/LICENSE.CDDL"; do
  test -f "$file" || { echo "missing RAW runtime input: $file" >&2; exit 1; }
done

cargo build --release -p lumia-plugin-raw
rm -rf "$STAGING_DIR"
mkdir -p "$PLUGIN_DIR/licenses"
cp target/release/lumia-plugin-raw "$PLUGIN_DIR/"
cp -L "$BRIDGE_LIBRARY" "$PLUGIN_DIR/"
cp -L "$LIBRAW_LIBRARY" "$PLUGIN_DIR/"
cp plugins/lumia-plugin-raw/lumia.plugin.json "$PLUGIN_DIR/"
cp plugins/lumia-plugin-raw/THIRD_PARTY_NOTICES.md "$PLUGIN_DIR/"
cp "$LIBRAW_LICENSE_DIRECTORY/LICENSE.LGPL" "$PLUGIN_DIR/licenses/"
cp "$LIBRAW_LICENSE_DIRECTORY/LICENSE.CDDL" "$PLUGIN_DIR/licenses/"
chmod +x "$PLUGIN_DIR/lumia-plugin-raw"

APP_VERSION="$(awk -F'"' '/^version = / { print $2; exit }' crates/lumia-app/Cargo.toml)"
PLUGIN_API_VERSION="$(awk '/PROTOCOL_VERSION: u32 = / { gsub(";", "", $6); print $6; exit }' crates/lumia-plugin-api/src/rpc.rs)"
node scripts/sign-plugin-package.mjs \
  --root "$STAGING_DIR" \
  --install-directory lumia-plugin-raw \
  --plugin-id lumia.raw \
  --target-os "$PLATFORM" \
  --target-arch "$ARCH" \
  --minimum-lumia-version "$APP_VERSION" \
  --plugin-api-version "$PLUGIN_API_VERSION"

for file in lumia-plugin-raw lumia.plugin.json lumia.plugin.sig \
  THIRD_PARTY_NOTICES.md licenses/LICENSE.LGPL licenses/LICENSE.CDDL; do
  test -f "$PLUGIN_DIR/$file"
done
test -f "$STAGING_DIR/lumia.package.json"
test -f "$STAGING_DIR/lumia.package.sig"
rm -f "$ARCHIVE"
(
  cd "$STAGING_DIR"
  zip -qr "$ROOT_DIR/$ARCHIVE" .
)
echo "Created $ARCHIVE"
