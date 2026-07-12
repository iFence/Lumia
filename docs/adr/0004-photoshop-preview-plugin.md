# ADR 0004: Decode Photoshop Documents in an Official Process Plugin

## Status

Accepted

## Context

Lumia needs default preview support for Adobe Photoshop PSD and PSB documents while preserving fast startup, low memory use, stability, and a small core viewer. Photoshop documents are complex, may be very large, and are untrusted binary inputs. The existing plugin protocol already models path-based probing and preview decode.

## Decision

- Add an official bundled `lumia.photoshop` process plugin for PSD/PSB composite preview.
- Use the same manifest, permissions, and newline-delimited JSON-RPC protocol as third-party plugins.
- Keep Photoshop-specific parsing dependencies out of `lumia-core` and `lumia-app`.
- Use `ag-psd` inside the plugin for pure-Rust PSD/PSB parsing.
- Exchange image payloads through host-selected paths. The plugin writes a bounded PNG preview.
- Retain the original PSD/PSB path in `ViewerSession`; temporary preview files are implementation details.
- Validate signatures and resource limits before decode, and treat plugin failure as recoverable.
- Limit the first release to a stored composite preview and basic metadata. Do not expose or re-render layers.

## Consequences

### Positive

- A malformed or unsupported Photoshop document cannot directly crash the GPUI process.
- Heavy parsing dependencies do not affect the common image-viewer startup path.
- PSD and PSB share a generic protocol that future professional-format plugins can reuse.
- Third-party and official plugins remain architecturally consistent.

### Negative

- Preview requires process startup and a temporary PNG, adding latency and disk I/O.
- `ag-psd` is young and does not support every Photoshop color mode or feature.
- Packaging must place the plugin executable and manifest where the app can discover them.

### Neutral

- Cache cleanup and plugin lifecycle management become application responsibilities.
- Layer browsing can be added later only through an explicit protocol extension.

## Alternatives Considered

**Decode in `lumia-core`**

Rejected because it violates ADR 0001 and expands the dependency, memory, and crash surface of the core viewer.

**Use `psd` or `zune-psd`**

Rejected for the initial implementation because neither provides the selected PSD/PSB coverage. They remain possible fallback decoders if future compatibility testing justifies a decoder chain.

**Use a C++ or Python helper**

Rejected because the extra runtime and native packaging complexity are not justified for flattened preview.

## References

- `docs/adr/0001-core-and-plugin-boundary.md`
- `docs/plans/2026-07-11-photoshop-preview-design.md`
