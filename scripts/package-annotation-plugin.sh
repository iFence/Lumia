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
ARCHIVE="target/$PACKAGE_NAME.tar.gz"

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

tar -czf "$ARCHIVE" -C "$STAGING_DIR" .
echo "Created $ARCHIVE"
