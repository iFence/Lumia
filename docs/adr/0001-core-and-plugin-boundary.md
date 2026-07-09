# ADR 0001: Core App and Process Plugin Boundary

## Status

Accepted

## Context

Lumia targets a small, polished, high-performance image-viewer core with a large extension surface. The product needs to serve ordinary users as well as photographers, UI designers, and engineers. The core browsing path must prioritize startup time, first-open latency, memory use, stability, and predictable cross-platform packaging.

The desired capability set spans several very different risk profiles: fast preview for common formats; EXIF and image information; folder browsing; lightweight copy-export editing; RAW, HDR, HEIC/HEIF, and other professional format previews; conversion; compression; batch workflows; local AI; and cloud AI. These capabilities should not all share the same dependency, crash, security, or release boundary.

GPUI is suitable for a high-performance desktop UI, but it is still pre-1.0. The app should avoid coupling UI development to heavy native dependencies or plugin implementation details.

## Decision

- Build the repository as a Cargo workspace.
- Keep the main GPUI application in `lumia-app`.
- Put reusable viewer state and domain models in `lumia-core`.
- Put all plugin wire types in `lumia-plugin-api`.
- Run plugins as separate processes using newline-delimited stdio JSON-RPC.
- Pass large image inputs and outputs by path and metadata.
- Keep the core viewer responsible for the fast path: preview, zoom, pan, display rotation, image information, folder navigation, and lightweight state.
- Keep built-in editing limited to lightweight copy-export operations such as rotate, crop, mirror, resize, simple compression, simple color adjustment, and export copy.
- Provide official bundled plugins for RAW, HDR, HEIC/HEIF, advanced/professional format preview, and simple format conversion when those capabilities are expected in the default product.
- Keep AI, cloud model access, local model access, batch watermarking, batch conversion, advanced compression, and heavyweight image processing in optional plugins.
- Require official bundled plugins and third-party plugins to use the same manifest, permission, and JSON-RPC protocol.

## Consequences

- The core app can remain small, fast, easier to package, and easier to keep stable.
- Users can still receive professional/default format support through bundled plugins without moving heavy dependencies into the core process.
- Plugins can crash or carry heavy dependencies without taking down the UI process by default.
- Protocol design matters early because it becomes the compatibility boundary for both bundled and third-party plugins.
- Some high-throughput operations may later need shared memory, a native helper, or a more specialized transport, but v1 avoids that complexity.
- Existing HEIC/HEIF core decode support is a transition bridge rather than a precedent for adding more heavy decoders to `lumia-core`.
