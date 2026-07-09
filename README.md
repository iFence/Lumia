# Lumia

Lumia is a small, polished, high-performance, cross-platform image viewer built with Rust, GPUI, and `gpui-component`.

The product goal is a viewer that opens quickly, stays low-memory, and remains stable while serving both everyday image browsing and professional preview workflows for photographers, UI designers, and engineers. The core app owns the desktop shell, viewer state, fast navigation, and plugin host. Heavier capabilities are isolated behind process plugins so they can evolve without slowing down or destabilizing the core viewer.

## Capability Model

Lumia is organized around four capability layers:

| Layer | Included capabilities | Architecture boundary |
|---|---|---|
| Core viewer | Image preview; zoom, pan, and display rotation; image information; EXIF display; folder browsing; basic sorting, filtering, and favorites; fast preview for common formats | Built into the app and optimized for startup time, open latency, memory use, and stability |
| Built-in light editing | Rotate, crop, mirror, resize, simple compression, simple color adjustments, and export copy | Built in only when the operation is lightweight and copy-export oriented |
| Official bundled plugins | RAW, HDR, HEIC/HEIF, professional/advanced format preview, and simple format conversion | Shipped with default builds, but implemented through the plugin protocol |
| Optional plugins | AI stylization, background removal, super-resolution, repair, outpainting, denoising, batch watermarking, batch conversion, compression plugins, cloud model plugins, and local model plugins | Installed or enabled separately through the same process-plugin boundary |

Current implementation status: single-image preview, zoom, pan, display rotation, image information, sibling-image navigation, adjacent preloading, settings, and the initial stdio JSON-RPC plugin protocol are in place. EXIF, full folder browsing UI, favorites, filtering, built-in edit tools, bundled professional-format plugins, and AI/batch plugins are product goals rather than complete features.

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

- `crates/lumia-app`: GPUI desktop shell, `gpui-component`-backed UI, viewer orchestration, and plugin-host integration.
- `crates/lumia-core`: viewer state and shared domain models with no UI dependency.
- `crates/lumia-plugin-api`: JSON-RPC types shared by the host and plugins.
- `crates/lumia-plugin-host`: process plugin launcher and newline-delimited stdio transport.
- `plugins/lumia-plugin-sample`: minimal process plugin used to validate the protocol.

## Architecture

The core application should remain the fast path for opening and browsing images. Common viewer state belongs in `lumia-core`; UI and event handling belong in `lumia-app`; plugin wire types belong in `lumia-plugin-api`; plugin process management belongs in `lumia-plugin-host`.

Image payloads cross the plugin boundary by path and metadata, not by base64 or JSON-inline pixel buffers. Official bundled plugins and third-party plugins use the same manifest, permission, and JSON-RPC protocol. This keeps professional formats, AI, cloud integrations, batch processing, and heavy native dependencies outside the core process.

`lumia-core` currently contains HEIC/HEIF decode support as a transition bridge. New heavy or professional format support should be designed as official bundled plugins unless a future ADR explicitly moves a narrow capability into core.

## UI Stack

- `gpui` and `gpui_platform` are sourced from the `zed-industries/zed` repository.
- `gpui-component` is pulled from `longbridge/gpui-component` and initialized in `crates/lumia-app/src/main.rs` with `gpui_component::init(cx)`.
- The direct `gpui` and `gpui_platform` dependencies intentionally use the same unpinned git URL shape as `gpui-component`; the actual Zed revision is pinned through `Cargo.lock`.
- The Lumia root view is wrapped in `gpui_component::Root`, and shared widgets in `crates/lumia-app/src/widgets.rs` use `gpui-component` primitives where that library fits the interaction model.

## Development

```powershell
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo run -p lumia-app
```

Notes:

- Updating GPUI means updating the Zed-sourced dependency set through Cargo, committing the resulting `Cargo.lock`, keeping `rust-toolchain.toml` aligned with the locked revision, and recording meaningful policy changes in an ADR.
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

The MSI is intended to include:
- Application installed to `Program Files`
- Start Menu and Desktop shortcuts
- File associations for registered image formats
- Clean uninstall via Windows "Apps & features"

### Releasing

Push a `v*` tag to trigger the CI release workflow:

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions will build the MSI, portable zip, and platform binaries, then attach them to a new [Release](https://github.com/iFence/Lumia/releases).

## Image Format Strategy

| Category | Extensions | Intended support path |
|---|---|---|
| Common web and desktop formats | `.jpg` `.jpeg` `.png` `.gif` `.webp` `.bmp` `.ico` `.tga` `.tif` `.tiff` | Core viewer fast path where dependencies stay lightweight |
| Additional lightweight formats | `.avif` `.dds` `.ff` `.farbfeld` `.pbm` `.pam` `.ppm` `.pgm` `.qoi` `.svg` | Core or plugin depending on dependency and rendering cost |
| Professional and heavy preview formats | `.hdr` `.exr` `.heic` `.heif` plus future RAW formats | Official bundled plugins by default |
| Conversion and batch output formats | Project-defined per plugin | Plugin protocol |

Current registered extensions include 24 extensions across 17 format families. Registration does not mean every advanced format should remain implemented inside the core app.
