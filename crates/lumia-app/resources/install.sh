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

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

install_lumia() {
    echo "Installing Lumia..."

    mkdir -p "$BIN_DIR" "$APPS_DIR" "$ICONS_DIR"

    # Binary
    if [ -f "$SCRIPT_DIR/lumia-app" ]; then
        cp "$SCRIPT_DIR/lumia-app" "$BIN_DIR/lumia-app"
        chmod +x "$BIN_DIR/lumia-app"
        echo "  ✓ Binary installed to $BIN_DIR/lumia-app"
    else
        echo "  ! lumia-app not found in current directory — skipping binary copy"
    fi

    # Desktop entry (with absolute path to binary)
    if [ -f "$SCRIPT_DIR/lumia.desktop" ]; then
        sed "s|Exec=lumia-app|Exec=${BIN_DIR}/lumia-app|" \
            "$SCRIPT_DIR/lumia.desktop" > "$APPS_DIR/lumia.desktop"
        echo "  ✓ Desktop entry installed to $APPS_DIR/lumia.desktop"
    else
        echo "  ! lumia.desktop not found — skipping"
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

    rm -f "$BIN_DIR/lumia-app"
    rm -f "$APPS_DIR/lumia.desktop"
    rm -f "$ICONS_DIR/lumia.png"

    if command -v update-desktop-database &>/dev/null; then
        update-desktop-database "$APPS_DIR" 2>/dev/null || true
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
