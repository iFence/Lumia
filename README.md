# Lumia

<p align="center">
  <img src="crates/lumia-app/resources/logo.png" alt="Lumia Logo" width="256">
</p>

<p align="center">
  <a href="https://github.com/iFence/Lumia/releases"><img src="https://img.shields.io/github/v/release/iFence/Lumia?style=flat-square&color=blue" alt="Release"></a>
  <a href="https://github.com/iFence/Lumia/releases"><img src="https://img.shields.io/github/downloads/iFence/Lumia/total?style=flat-square&color=green" alt="Downloads"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/iFence/Lumia?style=flat-square&color=orange" alt="License"></a>
  <img src="https://img.shields.io/badge/MSRV-1.95-red?style=flat-square" alt="MSRV">
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?style=flat-square" alt="Platform">
  <a href="https://github.com/iFence/Lumia"><img src="https://img.shields.io/github/stars/iFence/Lumia?style=social" alt="Stars"></a>
</p>

<p align="center">
  <a href="README_CN.md">中文版</a>
</p>

Lumia is a small, polished, high-performance, cross-platform image viewer built with Rust, GPUI, and `gpui-component`.

The product goal is a viewer that opens quickly, stays low-memory, and remains stable while serving both everyday image browsing and professional preview workflows for photographers, UI designers, and engineers. The core app owns the desktop shell, viewer state, fast navigation, and plugin host. Heavier capabilities are isolated behind process plugins so they can evolve without slowing down or destabilizing the core viewer.

## Capability Model

Lumia is organized around four capability layers:

| Layer | Included capabilities | Architecture boundary |
|---|---|---|
| Core viewer | Image preview; zoom, pan, and display rotation; image information; EXIF display; folder browsing; basic sorting, filtering, and favorites; fast preview for common formats | Built into the app and optimized for startup time, open latency, memory use, and stability |
| Built-in light editing | Rotate, crop, mirror, resize, simple compression, simple color adjustments, and export copy | Built in only when the operation is lightweight and copy-export oriented |
| Official plugins | Bundled PSD/PSB, JPEG XL, and JPEG 2000 preview; optional RAW preview; and future HDR, HEIC/HEIF, advanced-format preview, and simple format conversion | Implemented through the process-plugin protocol; each plugin may be bundled or released separately according to its size and dependencies |
| Optional plugins | AI stylization, background removal, super-resolution, repair, outpainting, denoising, batch watermarking, batch conversion, compression plugins, cloud model plugins, and local model plugins | Installed or enabled separately through the same process-plugin boundary |

Current implementation status: single-image preview, streaming GIF/APNG/animated WebP playback, zoom, pan, display rotation, image information, sibling-image navigation, adjacent preloading, settings, lightweight crop/resize copy export, the stdio JSON-RPC plugin protocol, bundled PSD/PSB, JPEG XL, and JPEG 2000 preview, optional signed RAW preview, and declarative plugin UI contributions are in place. Full folder browsing UI, favorites, filtering, additional professional-format plugins, and AI/batch plugins are product goals rather than complete features.

### Very large images

Common raster images that exceed Lumia's safe decoded-memory or GPU texture limits use an in-process progressive path. Lumia first creates a bounded preview, then prepares a disk-backed BGRA cache and loads only the visible 512×512 tiles as the user zooms or pans. PNG is processed row by row; formats whose current pure-Rust decoder requires a complete destination use a temporary memory-mapped file instead of a multi-gigabyte Rust heap allocation.

The cache is stored under the operating system temporary directory, is capped at 8 GiB, and removes incomplete or week-old entries on startup. Very large JPEG and WebP files can temporarily require disk space close to their decoded pixel size. Animated GIF, APNG, and WebP files whose frames exceed the safe animation budget display a bounded static preview through this progressive path.

## Installation

### Windows

Download the recommended Setup program (`Lumia-Setup-*-x64.exe`) or portable archive (`lumia-portable-windows-x64.zip`) from the [Releases](https://github.com/iFence/Lumia/releases) page.

- **Setup (recommended)**: Choose 简体中文 or English, then follow the installer. Lumia and its official Photoshop, JPEG XL, and JPEG 2000 preview plugins are installed for the current user under `%LOCALAPPDATA%\Programs\Lumia`, so administrator permission is not normally required. A Start Menu shortcut is always created; the optional Desktop shortcut is off by default. Setup also removes a detected legacy `Program Files` installation before continuing.
- **MSI packages**: Separate `en-US` and `zh-CN` MSI files are available for silent deployment and troubleshooting. They use the same per-user defaults, but migration from the legacy per-machine MSI must be performed with Setup or by uninstalling the old version first.
- **Portable**: Extract the complete `.zip` archive and run `lumia-app.exe`. Keep the included `plugins` directory beside the application. To add right-click support, run `lumia-app --register-context-menu` once.

### macOS

Download the `.dmg` from the [Releases](https://github.com/iFence/Lumia/releases) page. Open the disk image and drag **Lumia.app** into your `Applications` folder. Once installed, right-click any image in Finder and choose **Open With -> Lumia**, or use **Settings -> File Associations** in Lumia to choose the default formats.

If macOS reports that **Lumia.app is damaged and can't be opened**, open Terminal and run the following command, then launch Lumia again:

```bash
sudo xattr -dr com.apple.quarantine /Applications/Lumia.app
```

Only run this command for Lumia downloaded from the official [Releases](https://github.com/iFence/Lumia/releases) page.

If you prefer the portable binary, run `lumia-app --register-context-menu` to create a wrapper app bundle under `~/Applications/` so Lumia appears in Finder's "Open With" menu.

### Linux

Download the tarball (`lumia-linux-x64.tar.gz`) from the [Releases](https://github.com/iFence/Lumia/releases) page and extract it:

```bash
tar -xzf lumia-linux-x64.tar.gz
cd lumia-release
```

Run the included `install.sh` script to install the application, official plugins, desktop entry, and icon:

```bash
./install.sh
```

This registers Lumia in your system's right-click "Open With" menu for all supported image formats. Use **Settings -> File Associations** to make Lumia the default for selected formats. To uninstall, run `./install.sh --uninstall`.

If you've downloaded only the raw application binary, PSD/PSB, JPEG XL, and JPEG 2000 preview are unavailable because the official plugins are not present. You can still register the core viewer manually:

```bash
lumia-app --register-context-menu      # adds .desktop entry and icon
lumia-app --unregister-context-menu    # removes them
```

> **Linux dependencies**: GPU drivers, `xdg-utils`, and system libraries (fontconfig, wayland, xkbcommon, xcb) must be installed. On Debian/Ubuntu:
> ```bash
> sudo apt install xdg-utils shared-mime-info libfontconfig-dev libwayland-dev libxkbcommon-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-x11-dev
> ```

## File Associations and Default Applications

Lumia integrates with the operating system's "Open With" menu and default application settings. After choosing formats under **Settings -> File Associations**, double-clicking those images opens them in Lumia. If Lumia is already running, the existing window is activated and loads the new image.

| Platform | Default-application behavior |
|---|---|
| **Windows** | Registers selected formats under the current user and opens Windows Default Apps for confirmation. |
| **macOS** | Applies selected Viewer handlers through Launch Services. |
| **Linux** | Applies selected MIME handlers through `xdg-mime`. |

Windows 10 and 11 require the user to confirm default applications in system Settings. Lumia registers the selected formats and opens its Windows Default Apps page; it does not overwrite the protected user choice directly.

On macOS and Linux, clearing a format restores the handler captured when Lumia first took ownership, but only if Lumia is still the current default. Lumia never overwrites a default application changed externally. If no previous handler is known, the settings page identifies the formats that need manual selection.

## Official Bundled Plugins

The bundled Photoshop, JPEG XL, and JPEG 2000 preview plugins are installed, upgraded, and removed together with Lumia. Users do not need to download or copy them separately. Release artifacts preserve this application-relative layout:

```text
Lumia/
  lumia-app[.exe]
  plugins/
    lumia-plugin-photoshop/
      lumia-plugin-photoshop[.exe]
      lumia.plugin.json
    lumia-plugin-jpeg-xl/
      lumia-plugin-jpeg-xl[.exe]
      lumia.plugin.json
    lumia-plugin-jpeg2000/
      lumia-plugin-jpeg2000[.exe]
      lumia.plugin.json
```

The MSI, Windows portable ZIP, macOS app bundle, and Linux archive all contain this layout. The official RAW and Annotation plugins are published separately as signed `.lumiaplugin` packages and can be installed from the community plugin browser, exactly like third-party plugins.

JPEG XL support currently covers SDR still images; HDR JPEG XL is rejected until Lumia gains an HDR-capable display pipeline. JPEG 2000 support covers JP2 and Part 1 still-image codestreams (`.jp2`, `.j2k`, `.j2c`, and `.jpc`), not JPX, JPM, or Motion JPEG 2000.

## Optional RAW Plugin

The official `lumia.raw` plugin adds process-isolated camera RAW preview without placing LibRaw or another heavy decoder in Lumia's core process. It is released as a separate signed `.lumiaplugin` package and is not included in the default Lumia installer or portable archive. Lumia still recognizes supported RAW files when the plugin is absent and displays installation and restart guidance instead of treating them as unknown images.

The plugin uses LibRaw 0.22.2 to decode an orientation-corrected, 8-bit sRGB PNG preview whose longest edge is at most 4096 pixels. It also maps available camera make and model, lens, ISO, shutter speed, aperture, focal length, capture time, and GPS coordinates into Lumia's image information panel.

The signed Windows plugin package contains LibRaw and all of its native decoder dependencies. Users only install the `.lumiaplugin` package; no separate LibRaw, zlib, JPEG, or Microsoft Visual C++ runtime installation is required.

Supported extensions are matched case-insensitively:

`.dng`, `.cr2`, `.cr3`, `.crw`, `.nef`, `.nrw`, `.arw`, `.sr2`, `.srf`, `.raf`, `.orf`, `.rw2`, `.rwl`, `.pef`, `.srw`, `.3fr`, `.fff`, `.mef`, `.mos`, `.mrw`, `.kdc`, `.dcr`, `.erf`, `.x3f`, and `.iiq`.

RAW support is read-only in this first release. Browsing, zoom, pan, display rotation, and image information remain available, while editing, annotation, and exports derived from preview pixels are disabled so the preview is never mistaken for full-resolution source data.

To install the RAW plugin:

1. Open **Settings -> Plugins** and switch to the **Community** tab.
2. Search for **RAW**, review the plugin card, and choose **Install**.
3. Review the plugin's identity and permissions in the confirmation dialog, then choose **Install**.
4. Restart Lumia and open a supported RAW file.

If you are offline, you can still install the same signed package from the GitHub Release: download the `Lumia-RAW-<version>-<platform>-<architecture>.lumiaplugin` asset matching your system, then use **Install from file** in **Settings -> Plugins**.

Remove or upgrade the plugin from the same **Settings -> Plugins** page. Package and payload signatures, target platform, plugin API compatibility, paths, sizes, and SHA-256 digests are verified before installation.

## Optional Annotation Plugin

The official Annotation plugin is released as a separate package. Without it, Lumia contributes no annotation row to the image context menu and creates no annotation panel. Once installed and Lumia is restarted, right-click an image and choose **Annotate / 标注** to open the host-rendered panel, place icon markers, undo or redo changes, and export a PNG, JPEG, or WebP copy without changing the source image.

1. Open **Settings -> Plugins** and switch to the **Community** tab.
2. Search for **Annotation**, review the plugin card, and choose **Install**.
3. Review the plugin's identity, version, and requested permissions in the confirmation dialog, then choose **Install**.
4. Restart Lumia. Right-click an image and choose **Annotate / 标注**.

If you are offline, you can still install the same signed package from the GitHub Release: download the `Lumia-Annotation-<version>-<platform>-<architecture>.lumiaplugin` asset matching your system, then use **Install from file** in **Settings -> Plugins**.

Remove the plugin from the same **Settings -> Plugins** page. Removal hides its
contributed commands immediately; restart Lumia if the page asks you to finish
applying the change.

Lumia verifies the package signature, target OS and architecture, Lumia/plugin
API compatibility, every payload path, file size, and SHA-256 digest before
installation. Only packages signed by Lumia's official key are installed;
packages for a different platform are rejected without being installed.

Manual copying is no longer required. For troubleshooting, user-installed
plugins are stored below these fixed directories:

| Platform | Plugin directory |
|---|---|
| Windows | `%APPDATA%\Lumia\plugins\` |
| macOS | `~/Library/Application Support/Lumia/plugins/` |
| Linux | `$XDG_DATA_HOME/lumia/plugins/`, or `~/.local/share/lumia/plugins/` by default |

Existing manually copied official plugin directories remain discoverable, but
new installations should use Settings so package integrity and transactional
replacement are enforced.

### Release signing

Official release jobs require the protected GitHub Actions secret
`LUMIA_PLUGIN_SIGNING_KEY_PEM`. It may contain the official Ed25519 PKCS#8 PEM
or its base64-encoded DER form and must match the public key embedded in Lumia.
The signing script never prints the secret. A missing or mismatched key fails
packaging, and Lumia's production verifier checks the final `.lumiaplugin`
archive again before GitHub Release upload.

After the release assets are uploaded, the `update-community-index` job
regenerates the community `plugins.json` index from those assets and pushes it
to the `awesome-lumia-plugin` repository, so the RAW and Annotation plugins
appear in the community browser. This step uses the separate
`LUMA_INDEX_ACCESS_TOKEN` secret (a fine-grained PAT with Contents read/write on
`iFence/awesome-lumia-plugin`). It is best-effort: a missing or invalid token
warns in the job log but does not fail the release — the signed packages are
already on the Release, and the index can be refreshed later by re-running the
job.

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
- `plugins/lumia-plugin-photoshop`: official bundled PSD/PSB composite-preview plugin.
- `plugins/lumia-plugin-jpeg-xl`: official bundled JPEG XL still-image preview plugin.
- `plugins/lumia-plugin-jpeg2000`: official bundled JPEG 2000 Part 1 preview plugin.
- `plugins/lumia-plugin-raw`: optional official LibRaw-backed camera RAW preview plugin, including the native bridge sources.
- `plugins/lumia-plugin-annotation`: optional official icon-annotation plugin and signed package metadata.

## Architecture

The core application should remain the fast path for opening and browsing images. Common viewer state belongs in `lumia-core`; UI and event handling belong in `lumia-app`; plugin wire types belong in `lumia-plugin-api`; plugin process management belongs in `lumia-plugin-host`.

Image payloads cross the plugin boundary by path and metadata, not by base64 or JSON-inline pixel buffers. Official bundled plugins and third-party plugins use the same manifest, permission, and JSON-RPC protocol. This keeps professional formats, AI, cloud integrations, batch processing, and heavy native dependencies outside the core process.

UI-capable plugins contribute commands, context-menu rows, panels, controls, and canvas-tool declarations as bounded protocol data. Plugins cannot inject GPUI elements or arbitrary HTML. Lumia renders every contribution, owns pointer-rate canvas interaction, validates returned panel models, and terminates timed-out or malformed sessions.

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

Build both localized MSI packages and the Setup bootstrapper:

```powershell
$env:WIX = "$env:LOCALAPPDATA\wixtoolset"
./scripts/build-windows-installers.ps1
# Outputs:
# target/wix/Lumia-Setup-<version>-x64.exe
# target/wix/Lumia-<version>-x64-en-US.msi
# target/wix/Lumia-<version>-x64-zh-CN.msi
```

The installers include:
- Per-user installation under `%LOCALAPPDATA%\Programs\Lumia`
- A required Start Menu shortcut and optional Desktop shortcut
- English and Simplified Chinese installer UI
- Per-user file associations managed from Lumia settings
- Clean uninstall via Windows "Apps & features"

Validate the ICO structure and generated packages with:

```powershell
./scripts/verify-windows-icon.ps1
./scripts/verify-windows-packages.ps1 -PackageDirectory target/wix
```

### Releasing

Push a `v*` tag to trigger the CI release workflow:

```bash
git tag v0.2.2
git push origin v0.2.2
```

GitHub Actions will build the Setup EXE, both localized MSI packages, portable zip, platform binaries, and separate signed Annotation and RAW plugin archives, then verify and attach them to a new [Release](https://github.com/iFence/Lumia/releases).

## Image Format Strategy

| Category | Extensions | Intended support path |
|---|---|---|
| Common web and desktop formats | `.jpg` `.jpeg` `.png` `.apng` `.gif` `.webp` `.bmp` `.ico` `.tga` `.tif` `.tiff` | Core viewer fast path where dependencies stay lightweight; GIF, APNG, and animated WebP use streaming playback |
| Additional lightweight formats | `.avif` `.dds` `.ff` `.farbfeld` `.pbm` `.pam` `.ppm` `.pgm` `.qoi` `.svg` | Core or plugin depending on dependency and rendering cost |
| Professional and heavy preview formats | `.hdr` `.exr` `.heic` `.heif` `.jxl` `.jp2` `.j2k` `.j2c` `.jpc` `.psd` `.psb` and the camera RAW extensions listed above | JPEG XL, JPEG 2000, and PSD/PSB use bundled process plugins; RAW uses the optional signed `lumia.raw` plugin |
| Conversion and batch output formats | Project-defined per plugin | Plugin protocol |

Current registered extensions include 57 extensions across 21 format families. PSD/PSB support previews the stored composite image through the bundled process plugin; it does not expose layers or edit Photoshop documents. JPEG XL and JPEG 2000 provide bounded still-image previews through their bundled plugins. RAW support uses the optional process plugin described above and remains read-only. Registration does not mean every advanced format should be implemented inside the core app.

Build the application and bundled preview plugins together with:

    cargo build --release -p lumia-app -p lumia-plugin-photoshop -p lumia-plugin-jpeg-xl -p lumia-plugin-jpeg2000

The executables are emitted into the same target profile directory so Lumia can discover the plugins beside the application.
