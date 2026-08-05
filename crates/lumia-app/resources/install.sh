#!/usr/bin/env bash
set -euo pipefail

# Lumia Linux installer — registers the app in the system's right-click
# "Open With" menu by installing the binary, .desktop entry, and icon.
#
# Usage:
#   ./install.sh            Install Lumia
#   ./install.sh --uninstall  Remove Lumia

BIN_DIR="${HOME}/.local/bin"
APPS_DIR="${HOME}/.local/share/applications"
ICONS_DIR="${HOME}/.local/share/icons/hicolor/128x128/apps"
MIME_PACKAGES_DIR="${HOME}/.local/share/mime/packages"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PLUGIN_SOURCE_DIR="$SCRIPT_DIR/plugins/lumia-plugin-photoshop"
PLUGIN_INSTALL_DIR="$BIN_DIR/plugins/lumia-plugin-photoshop"
JPEG_XL_PLUGIN_SOURCE_DIR="$SCRIPT_DIR/plugins/lumia-plugin-jpeg-xl"
JPEG_XL_PLUGIN_INSTALL_DIR="$BIN_DIR/plugins/lumia-plugin-jpeg-xl"
JPEG2000_PLUGIN_SOURCE_DIR="$SCRIPT_DIR/plugins/lumia-plugin-jpeg2000"
JPEG2000_PLUGIN_INSTALL_DIR="$BIN_DIR/plugins/lumia-plugin-jpeg2000"

install_lumia() {
    echo "Installing Lumia..."

    if [ ! -f "$SCRIPT_DIR/lumia-app" ]; then
        echo "  ! lumia-app is missing from the release archive" >&2
        return 1
    fi
    if [ ! -f "$PLUGIN_SOURCE_DIR/lumia-plugin-photoshop" ]; then
        echo "  ! Photoshop preview plugin is missing from the release archive" >&2
        return 1
    fi
    if [ ! -f "$PLUGIN_SOURCE_DIR/lumia.plugin.json" ]; then
        echo "  ! Photoshop plugin manifest is missing from the release archive" >&2
        return 1
    fi
    if [ ! -f "$JPEG_XL_PLUGIN_SOURCE_DIR/lumia-plugin-jpeg-xl" ]; then
        echo "  ! JPEG XL preview plugin is missing from the release archive" >&2
        return 1
    fi
    if [ ! -f "$JPEG_XL_PLUGIN_SOURCE_DIR/lumia.plugin.json" ]; then
        echo "  ! JPEG XL plugin manifest is missing from the release archive" >&2
        return 1
    fi
    if [ ! -f "$JPEG2000_PLUGIN_SOURCE_DIR/lumia-plugin-jpeg2000" ]; then
        echo "  ! JPEG 2000 preview plugin is missing from the release archive" >&2
        return 1
    fi
    if [ ! -f "$JPEG2000_PLUGIN_SOURCE_DIR/lumia.plugin.json" ]; then
        echo "  ! JPEG 2000 plugin manifest is missing from the release archive" >&2
        return 1
    fi

    mkdir -p "$BIN_DIR" "$APPS_DIR" "$ICONS_DIR" "$MIME_PACKAGES_DIR"

    # Binary
    cp "$SCRIPT_DIR/lumia-app" "$BIN_DIR/lumia-app"
    chmod +x "$BIN_DIR/lumia-app"
    echo "  ✓ Binary installed to $BIN_DIR/lumia-app"

    # Official bundled plugins
    mkdir -p "$PLUGIN_INSTALL_DIR"
    cp "$PLUGIN_SOURCE_DIR/lumia-plugin-photoshop" "$PLUGIN_INSTALL_DIR/"
    cp "$PLUGIN_SOURCE_DIR/lumia.plugin.json" "$PLUGIN_INSTALL_DIR/"
    chmod +x "$PLUGIN_INSTALL_DIR/lumia-plugin-photoshop"
    echo "  ✓ Photoshop preview plugin installed to $PLUGIN_INSTALL_DIR"
    mkdir -p "$JPEG_XL_PLUGIN_INSTALL_DIR"
    cp "$JPEG_XL_PLUGIN_SOURCE_DIR/lumia-plugin-jpeg-xl" "$JPEG_XL_PLUGIN_INSTALL_DIR/"
    cp "$JPEG_XL_PLUGIN_SOURCE_DIR/lumia.plugin.json" "$JPEG_XL_PLUGIN_INSTALL_DIR/"
    chmod +x "$JPEG_XL_PLUGIN_INSTALL_DIR/lumia-plugin-jpeg-xl"
    echo "  ✓ JPEG XL preview plugin installed to $JPEG_XL_PLUGIN_INSTALL_DIR"
    mkdir -p "$JPEG2000_PLUGIN_INSTALL_DIR"
    cp "$JPEG2000_PLUGIN_SOURCE_DIR/lumia-plugin-jpeg2000" "$JPEG2000_PLUGIN_INSTALL_DIR/"
    cp "$JPEG2000_PLUGIN_SOURCE_DIR/lumia.plugin.json" "$JPEG2000_PLUGIN_INSTALL_DIR/"
    chmod +x "$JPEG2000_PLUGIN_INSTALL_DIR/lumia-plugin-jpeg2000"
    echo "  ✓ JPEG 2000 preview plugin installed to $JPEG2000_PLUGIN_INSTALL_DIR"

    # Desktop entry (with absolute path to binary)
    if [ -f "$SCRIPT_DIR/lumia.desktop" ]; then
        sed "s|Exec=lumia-app|Exec=${BIN_DIR}/lumia-app|" \
            "$SCRIPT_DIR/lumia.desktop" > "$APPS_DIR/lumia.desktop"
        echo "  ✓ Desktop entry installed to $APPS_DIR/lumia.desktop"
    else
        echo "  ! lumia.desktop not found — skipping"
    fi

    if [ -f "$SCRIPT_DIR/lumia-mime.xml" ]; then
        cp "$SCRIPT_DIR/lumia-mime.xml" "$MIME_PACKAGES_DIR/lumia-image-formats.xml"
        if command -v update-mime-database &>/dev/null; then
            update-mime-database "${HOME}/.local/share/mime" 2>/dev/null || true
        fi
        echo "  ✓ MIME definitions installed"
    fi

    # Icon
    if [ -f "$SCRIPT_DIR/icon.png" ]; then
        cp "$SCRIPT_DIR/icon.png" "$ICONS_DIR/lumia.png"
        echo "  ✓ Icon installed to $ICONS_DIR/lumia.png"
    else
        echo "  ! icon.png not found — skipping"
    fi

    # Refresh desktop database (non-fatal if missing)
    if command -v update-desktop-database &>/dev/null; then
        update-desktop-database "$APPS_DIR" 2>/dev/null || true
    fi

    echo "Done. Right-click any image and choose 'Open With → Lumia'."
}

uninstall_lumia() {
    echo "Uninstalling Lumia..."

    if [ -x "$BIN_DIR/lumia-app" ]; then
        "$BIN_DIR/lumia-app" --unregister-context-menu 2>/dev/null || true
    fi
    rm -f "$BIN_DIR/lumia-app"
    rm -rf "$PLUGIN_INSTALL_DIR"
    rm -rf "$JPEG_XL_PLUGIN_INSTALL_DIR"
    rm -rf "$JPEG2000_PLUGIN_INSTALL_DIR"
    rmdir "$BIN_DIR/plugins" 2>/dev/null || true
    rm -f "$APPS_DIR/lumia.desktop"
    rm -f "$ICONS_DIR/lumia.png"
    rm -f "$MIME_PACKAGES_DIR/lumia-image-formats.xml"

    if command -v update-desktop-database &>/dev/null; then
        update-desktop-database "$APPS_DIR" 2>/dev/null || true
    fi
    if command -v update-mime-database &>/dev/null; then
        update-mime-database "${HOME}/.local/share/mime" 2>/dev/null || true
    fi

    echo "Done. Lumia has been removed."
}

case "${1:-}" in
    --uninstall|-u)
        uninstall_lumia
        ;;
    *)
        install_lumia
        ;;
esac
