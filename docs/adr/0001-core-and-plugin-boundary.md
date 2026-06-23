# ADR 0001: Core App and Process Plugin Boundary

## Status

Accepted

## Context

Lumia targets a small image-viewer core with a large extension surface: 90+ preview formats, HDR, conversion, compression, crop/export, super-resolution, and cloud AI features. These capabilities have very different dependency, security, and performance profiles.

GPUI is suitable for a high-performance desktop UI, but it is still pre-1.0. The app should avoid coupling UI development to heavy native dependencies or plugin implementation details.

## Decision

- Build the repository as a Cargo workspace.
- Keep the main GPUI application in `lumia-app`.
- Put domain state in `lumia-core`.
- Put all plugin wire types in `lumia-plugin-api`.
- Run plugins as separate processes using newline-delimited stdio JSON-RPC.
- Pass large image inputs and outputs by path and metadata.
- Provide official plugins for broad format support and editing features instead of linking those dependencies into the core app.

## Consequences

- The core app can remain small and easier to package.
- Plugins can crash or carry heavy dependencies without taking down the UI process by default.
- Protocol design matters early because it becomes the compatibility boundary.
- Some high-throughput operations may later need shared memory or a native helper, but v1 avoids that complexity.
