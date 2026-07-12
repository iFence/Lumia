# ADR 0005: In-process large image decoding

## Status

Accepted

## Context

Very large common raster images can exceed both available heap memory and the GPU's single-texture limit.
Lumia needs bounded previews and on-demand tiles for PNG, JPEG, WebP, BMP, TIFF, and GIF. ADR 0001 places
heavy image processing behind process plugins by default, but product direction explicitly requires this large-
image path to remain in the main application and to use the existing pure-Rust dependency family.

## Decision

- Keep normal common-format images on the existing GPUI path-backed fast path.
- Select the in-process large-image path only after checked metadata exceeds dimension or decoded-byte limits.
- Put UI-independent geometry, decoding, and disk-raster primitives in `lumia-core`.
- Put GPUI scheduling, cancellation, LRU, and tiled composition in `lumia-app`.
- Reuse already locked pure-Rust format codecs and do not add libvips or another native image runtime.
- Stream formats whose codecs expose rows, rectangles, strips, chunks, or low-level frame data.
- For codecs that require a complete output buffer, use a checked disk-backed memory map rather than a
  multi-gigabyte Rust heap allocation.
- Decode outside the GPUI thread with bounded concurrency, pixel budgets, generation checks, and cache limits.
- Keep RAW, HDR, HEIF migration, AI, networking, batch processing, and professional formats under ADR 0001.

## Consequences

The user's large PNG can be previewed without one giant heap buffer or GPU texture, and common formats share
one progressive UI. Normal startup and browsing remain unchanged because the path is lazy.

The main process no longer has crash isolation for this narrow decoder path. JPEG and WebP can require temporary
disk space close to their decoded size, and first open may be slower than a native demand-driven backend. The
implementation must validate every size before allocation, check disk capacity, preserve a preview when tile work
fails, and keep format-specific code isolated so a better pure-Rust codec can replace it later.
