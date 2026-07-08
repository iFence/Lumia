# Lumia

Lumia is a small, cross-platform image viewer built with Rust on top of GPUI and `gpui-component`.

The core app is intentionally small: it owns the window, viewer state, task orchestration, and plugin host. GPUI provides the desktop runtime and rendering model, while `gpui-component` supplies shared controls such as buttons, dropdowns, menus, and the root UI wrapper. Heavy capabilities such as broad format support, compression, conversion, crop/export, super-resolution, and cloud AI editing are designed as process plugins.

## Installation

### Windows

Download the latest installer (`lumia-app-*-x64.msi`) or portable archive (`lumia-portable-windows-x64.zip`) from the [Releases](https://github.com/iFence/Lumia/releases) page.

- **MSI installer**: Run the `.msi` file. Lumia will be installed to `Program Files`, with Start Menu and Desktop shortcuts. Right-click any image in Explorer and choose "Open with Lumia" - no extra setup needed.
- **Portable**: Extract the `.zip` archive and run `lumia-app.exe`. To add right-click support, run `lumia-app --register-context-menu` once.

### macOS

Download the `.dmg` from the [Releases](https://github.com/iFence/Lumia/releases) page. Open the disk image and drag **Lumia.app** into your `Applications` folder. Once installed, right-click any image in Finder and choose **Open With -> Lumia**.

If you prefer the portable binary, run `lumia-app --register-context-menu` to create a wrapper app bundle under `~/Applications/` so Lumia appears in Finder's "Open With" menu.

### Linux

Download the tarball (`lumia-linux-x64.tar.gz`) from the [Releases](https://github.com/iFence/Lumia/releases) page and extract it:

```bash
tar -xzf lumia-linux-x64.tar.gz
cd lumia-release
```

Run the included `install.sh` script to install the binary, desktop entry, and icon:

```bash
./install.sh
```

This registers Lumia in your system's right-click "Open With" menu for all supported image formats. To uninstall, run `./install.sh --uninstall`.

If you've downloaded the raw binary without the tarball, you can register the context menu manually:

```bash
lumia-app --register-context-menu      # adds .desktop entry and icon
lumia-app --unregister-context-menu    # removes them
```

> **Linux dependencies**: GPU drivers and system libraries (fontconfig, wayland, xkbcommon, xcb) must be installed. On Debian/Ubuntu:
> ```bash
> sudo apt install libfontconfig-dev libwayland-dev libxkbcommon-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-x11-dev
> ```

## Right-Click Context Menu

Lumia integrates into your operating system's right-click / "Open With" menu so you can open images directly from your file manager.

| Platform | Mechanism | How to enable |
|---|---|---|
| **Windows** | Registry entries under `HKLM` (MSI) or `HKCU` (portable) | MSI: automatic. Portable: `lumia-app --register-context-menu` |
| **macOS** | `.app` bundle with `CFBundleDocumentTypes` in `Info.plist` | `.dmg`: drag to Applications. Portable: `lumia-app --register-context-menu` |
| **Linux** | `.desktop` file with `MimeType` entries | `./install.sh` from the tarball, or `lumia-app --register-context-menu` |

The `--register-context-menu` command is designed for portable / development use. It never requires administrator privileges:

- **Windows**: writes to `HKCU\Software\Classes` (current user registry hive)
- **macOS**: creates a minimal `.app` wrapper under `~/Applications/`
- **Linux**: writes `lumia.desktop` and icon to `~/.local/share/`

## Workspace

- `crates/lumia-app`: GPUI desktop shell and `gpui-component`-backed UI layer.
- `crates/lumia-core`: viewer state and shared domain models.
- `crates/lumia-plugin-api`: JSON-RPC types shared by the host and plugins.
- `crates/lumia-plugin-host`: process plugin launcher and stdio transport.
- `plugins/lumia-plugin-sample`: minimal process plugin used to validate the protocol.

## UI Stack

- `gpui` and `gpui_platform` are sourced from the `zed-industries/zed` repository instead of a crates.io-pinned `0.2.x` release.
- The workspace keeps a local compatibility patch set under `vendor/zed/`, wired through the root `Cargo.toml` `[patch."https://github.com/zed-industries/zed"]` section.
- `gpui-component` is pulled from `longbridge/gpui-component` and initialized in `crates/lumia-app/src/main.rs` with `gpui_component::init(cx)`.
- The Lumia root view is wrapped in `gpui_component::Root`, and shared widgets in `crates/lumia-app/src/widgets.rs` use `gpui-component` primitives where that library already fits the interaction model.

## Development

```powershell
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo run -p lumia-app
```

Notes:

- Updating GPUI now means updating the Zed-sourced dependency set and keeping the local `vendor/zed` patch entries in sync.
- When changing UI infrastructure, verify both the GPUI surface and the `gpui-component` integration still build cleanly across the workspace.

## Packaging

### Windows MSI installer

Install `cargo-wix` and download the WiX Toolset binaries (no admin required):

```powershell
# Install cargo-wix
cargo install cargo-wix --version 0.3.9

# Download and extract WiX Toolset binaries to %LOCALAPPDATA%
$url = "https://github.com/wixtoolset/wix3/releases/download/wix3141rtm/wix314-binaries.zip"
$dest = "$env:LOCALAPPDATA\wixtoolset"
Invoke-WebRequest -Uri $url -OutFile "$env:TEMP\wix314-binaries.zip"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Expand-Archive -Path "$env:TEMP\wix314-binaries.zip" -DestinationPath $dest -Force
```

Build the MSI:

```powershell
$env:WIX = "$env:LOCALAPPDATA\wixtoolset"
cargo wix -p lumia-app --output target/wix/
# Output: target/wix/lumia-app-0.1.0-x86_64.msi
```

The MSI includes:
- Application installed to `Program Files`
- Start Menu and Desktop shortcuts
- File associations for 24 image formats (double-click to open in Lumia)
- Clean uninstall via Windows "Apps & features"

### Releasing

Push a `v*` tag to trigger the CI release workflow:

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions will build the MSI, portable zip, and platform binaries, then attach them to a new [Release](https://github.com/iFence/Lumia/releases).

## Supported Image Formats

| Category | Extensions |
|---|---|
| AVIF | `.avif` |
| BMP | `.bmp` |
| DDS | `.dds` |
| EXR | `.exr` |
| Farbfeld | `.ff` `.farbfeld` |
| GIF | `.gif` |
| HDR / Radiance | `.hdr` |
| HEIC / HEIF | `.heic` `.heif` |
| ICO | `.ico` |
| JPEG | `.jpg` `.jpeg` |
| Netpbm | `.pbm` `.pam` `.ppm` `.pgm` |
| PNG | `.png` |
| QOI | `.qoi` |
| SVG | `.svg` |
| TGA | `.tga` |
| TIFF | `.tif` `.tiff` |
| WebP | `.webp` |

**24** extensions across **17** format families.
