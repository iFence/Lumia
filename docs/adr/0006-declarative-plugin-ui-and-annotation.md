# ADR 0006: Declarative plugin UI and host-owned canvas overlays

## Status

Accepted

## Context

Process plugins originally supported task-style requests such as probe and
preview decode. A graphical extension such as icon annotation also needs to
contribute a context-menu command, open a right-side panel, react to controls,
and draw over the current image. Loading plugin Rust code into the GPUI process
or accepting arbitrary HTML would weaken crash isolation, styling consistency,
input safety, and cross-platform maintainability.

The viewer must also keep pointer interaction and painting responsive if a
plugin process is slow or fails. Optional functionality must disappear
completely when its package is absent.

## Decision

- Protocol version 2 adds declarative commands, viewer context-menu rows,
  right-side panels, canvas tools, localized text, and package assets.
- Plugins return a bounded `PanelModel` made from host-supported controls.
  They cannot return GPUI elements, callbacks, scripts, or HTML.
- Lumia renders contributed UI with its own palette and widgets. Stable IDs are
  sent back through `ui.event`; the plugin responds with a replacement panel
  model.
- The host validates contribution counts, IDs, command references, localized
  strings, control ranges, selected values, colors, and asset references.
  Responses are capped at 1 MiB. Control requests use a five-second timeout;
  long-running decode tasks keep a separate, longer timeout.
- Canvas pointer mapping, overlay painting, undo/redo history, and copy export
  remain in the host process. The plugin supplies tool state and receives
  committed-operation notifications, so no JSON-RPC round trip occurs on every
  pointer movement or frame.
- UI plugins are discovered once at startup from the application plugin
  directory and the fixed per-user data directory. Installing or removing one
  therefore requires an application restart.
- This phase accepts only allowlisted official UI plugin IDs. Each package has
  an Ed25519 signature over the exact manifest bytes. Asset paths must remain
  inside the package and every asset must match its declared SHA-256 digest.
  The signing private key is not stored in the repository.
- The first implementation is `lumia-plugin-annotation`. It is released as an
  independent optional package, not included in the base application
  installer. If absent, it contributes no context-menu row or panel.
- Annotation export writes a new PNG, JPEG, or WebP file and never modifies the
  source image.

## Package layout

```text
lumia-plugin-annotation/
  lumia-plugin-annotation[.exe]
  lumia.plugin.json
  lumia.plugin.sig
  assets/
    pin.svg
    star.svg
    check.svg
```

The package is copied as one directory below the platform plugin root. Release
automation produces a separate archive for Windows, macOS, and Linux.

## Consequences

- Graphical plugins can add discoverable product UI without linking into the
  core process.
- The host retains control of frame-rate-sensitive work, theme consistency,
  accessibility evolution, and input validation.
- The current control vocabulary is intentionally finite. Adding a new control
  or canvas operation requires a protocol change and host implementation.
- General third-party trust, permission consent UI, package installation, and
  update management remain future work. The protocol shapes are shared, but
  the current loader intentionally rejects non-official IDs.
