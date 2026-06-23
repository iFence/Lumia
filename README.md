# Lumia

Lumia is a small, cross-platform image viewer built with Rust and GPUI.

The core app is intentionally small: it owns the window, viewer state, task orchestration, and plugin host. Heavy capabilities such as broad format support, compression, conversion, crop/export, super-resolution, and cloud AI editing are designed as process plugins.

## Workspace

- `crates/lumia-app`: GPUI desktop shell.
- `crates/lumia-core`: viewer state and shared domain models.
- `crates/lumia-plugin-api`: JSON-RPC types shared by the host and plugins.
- `crates/lumia-plugin-host`: process plugin launcher and stdio transport.
- `plugins/lumia-plugin-sample`: minimal process plugin used to validate the protocol.

## Development

```powershell
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo run -p lumia-app
```

GPUI is pinned to `=0.2.2`. Upgrades should be documented in `docs/adr/` because GPUI is still pre-1.0 and may change APIs.

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
| ICO | `.ico` |
| JPEG | `.jpg` `.jpeg` |
| Netpbm | `.pbm` `.pam` `.ppm` `.pgm` |
| PNG | `.png` |
| QOI | `.qoi` |
| SVG | `.svg` |
| TGA | `.tga` |
| TIFF | `.tif` `.tiff` |
| WebP | `.webp` |

**22** extensions across **16** format families.
