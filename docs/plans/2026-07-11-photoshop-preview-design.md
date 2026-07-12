# Photoshop PSD/PSB Composite Preview Design

## Status

Approved on 2026-07-11.

## Scope

Lumia will preview Adobe Photoshop PSD and PSB documents as flattened composite images. The first release includes format recognition, folder navigation, file associations, dimensions and basic color metadata, bounded background preview decoding, and normal viewer interactions. It does not expose layers, edit Photoshop documents, or reconstruct a missing composite from layers.

## Architecture

PSD/PSB parsing runs in an official bundled process plugin named `lumia.photoshop`. The GPUI process selects the plugin for `.psd` and `.psb`, sends newline-delimited JSON-RPC requests containing the source path and a host-owned temporary PNG path, and loads the generated PNG through the existing raster path. The viewer retains the original Photoshop path as its document source.

The plugin uses the generic `Probe` and `DecodePreview` capabilities. It validates the `8BPS` header and resource limits before invoking the pure-Rust, MIT-licensed `ag-psd` decoder. Plugin output is always path-based; pixels are never embedded in JSON.

## Data Flow

1. `lumia-app` recognizes a PSD/PSB path and creates an image document for the original source.
2. The professional-format route locates and validates the bundled plugin manifest.
3. The host starts the plugin and verifies protocol version, plugin identity, declared capabilities, supported media types, and required permissions.
4. `image.probe` returns format, dimensions, depth, and color-mode metadata.
5. `image.decode_preview` writes an aspect-preserving PNG preview to a host-selected cache path.
6. Lumia decodes the PNG to `PreparedImage`, applies the active load generation, and preloads adjacent professional images one at a time.

Cache identity is derived from normalized source path, file size, last-modified time, and requested preview bounds. Stale load generations ignore completed results. Incomplete outputs are removed after failures.

## Failure and Security Model

The plugin rejects invalid signatures, unsupported versions, files larger than 2 GiB, a side longer than 100,000 pixels, or more than 500 million pixels before full decode. Errors use stable categories: `unsupported_format`, `corrupt_image`, `resource_limit`, `decode_failed`, `cancelled`, `plugin_unavailable`, and `protocol_mismatch`.

Plugin crashes, malformed responses, and premature stdout closure cannot terminate the UI process. Lumia reports localized user-facing errors without exposing internal paths. The first release prioritizes RGB and grayscale documents with a valid composite. Unsupported CMYK, Lab, 32-bit, or missing-composite cases fail explicitly rather than attempting incomplete layer rendering.

## Alternatives Considered

- Adding a PSD decoder to `lumia-core` was rejected because it violates the heavy/professional decoder boundary and weakens crash isolation.
- The `psd` crate is mature and simple but does not cover the required PSB scope.
- `zune-psd` is lightweight but intentionally supports only a narrow subset of PSD.
- C++ or Python decoders increase packaging size and cross-platform operational complexity.

## Verification

Automated tests cover manifest validation, protocol mismatch, PSD/PSB header probing, corrupt and oversized inputs, preview output, cache identity, extension grouping, and preservation of the original viewer path. Manual testing covers PSD/PSB open, navigation, plugin absence/crash recovery, large-file feedback, and PNG/JPEG/HEIF regressions.
