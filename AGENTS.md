# AGENTS.md

## Project Intent

Lumia is a small, polished, high-performance, cross-platform image viewer for everyday users and professional users such as photographers, UI designers, and engineers. The product should feel fast for ordinary image browsing while still leaving room for professional preview formats and extensibility.

The core app must stay small, fast, low-memory, stable, and maintainable. Startup, first image open, folder navigation, zooming, panning, rotation, and metadata display are core quality bars. Heavy decoders, AI, networking, batch processing, and model SDKs must stay outside the core process unless an explicit ADR moves a narrowly scoped capability into core.

Product capability layers:

1. Core viewer: image preview; zoom, pan, and display rotation; image information; EXIF display; folder browsing; basic sorting, filtering, and favorites; fast preview for common formats.
2. Built-in light editing: non-destructive or copy-export operations only, including rotate, crop, mirror, resize, simple compression, simple color adjustments, and export copy.
3. Official bundled plugins: professional and heavier default capabilities such as RAW, HDR, HEIC/HEIF, advanced/professional format preview, and simple format conversion. Users may experience these as default support, but implementation should remain behind the plugin boundary.
4. Optional third-party or advanced plugins: AI stylization, background removal, super-resolution, repair, outpainting, denoising, batch watermarking, batch conversion, compression plugins, cloud model plugins, and local model plugins.

Current transition note: `lumia-core` currently contains HEIC/HEIF decode support. Treat this as a compatibility bridge, not a precedent for adding more heavy decoders to core. Future professional/heavy format work should move toward official bundled plugins.

## Workspace Structure

```
crates/
  lumia-core/src/              -- UI-independent viewer domain
    lib.rs                     -- module declarations and re-exports only
    image.rs + image/          -- facade; types, formats, loading, raster, HEIC bridge
    navigation.rs              -- FolderNavigation scanning and traversal
    viewer.rs                  -- ViewerSession and display-transform state
    viewport.rs                -- ViewportState, FitMode
    settings.rs, task.rs       -- settings and task models

  lumia-plugin-api/src/        -- pure plugin protocol data
  lumia-plugin-host/src/       -- process transport
    lib.rs                     -- declarations and re-exports only
    error.rs, process.rs       -- host errors and stdio JSON-RPC process

  lumia-app/src/               -- GPUI desktop integration
    main.rs, bootstrap.rs      -- CLI/action skeleton and GPUI window startup
    app.rs                     -- LumiaApp state composition and construction
    load_state.rs              -- load generations, queued preloads, decode/cache lifecycle
    image_loading.rs           -- decode, preload, and navigation orchestration
    viewer_actions.rs          -- open, zoom, rotate viewer commands
    window_actions.rs          -- fullscreen, panels, status hover behavior
    preferences.rs             -- settings updates and shortcut bindings
    ui_state.rs                -- pointer, window, menu, overlay, and settings-panel state
    render.rs                  -- root Render implementation and viewer surface
    status_bar.rs              -- status/navigation/zoom controls
    viewer_overlays.rs         -- zoom menu, decode overlay, context menu
    settings_ui.rs             -- settings panel shell and sidebar
    settings_general.rs        -- language/theme settings
    settings_shortcuts.rs      -- shortcut editor
    image_info.rs, widgets.rs  -- image overlay and shared widget factories
    palette.rs, i18n.rs        -- theme palette and translations
    persistence.rs, util.rs    -- settings storage and formatting helpers
    shell.rs + shell/          -- OS dispatch and per-platform registration

plugins/
  lumia-plugin-sample/         -- minimal stdin/stdout JSON-RPC plugin
```
## Crate Dependency Graph

```
lumia-app ──────> lumia-core
    │                  (无工作区依赖)
    └──────> lumia-plugin-host ──> lumia-plugin-api
                                            (无工作区依赖)

lumia-plugin-sample ──> lumia-plugin-api
```

- 无循环依赖
- `lumia-core` 和 `lumia-plugin-api` 是叶子 crate
- `lumia-app` 是唯一的整合点

## Architecture Rules

- Use Rust and GPUI for the desktop application.
- Keep UI code in `crates/lumia-app` thin; put reusable viewer state and task models in `crates/lumia-core`.
- Keep the core viewer path optimized for startup time, open latency, memory use, and crash isolation.
- Do not add heavy decoder, AI, networking, batch-processing, or model SDK dependencies to `lumia-app`.
- Do not add new heavy/professional format decoders to `lumia-core`; route them through official bundled plugins unless an ADR explicitly approves a core exception.
- Keep built-in editing constrained to lightweight copy-export operations. Complex edits, batch edits, and AI edits belong in plugins.
- All plugin-facing request and response shapes must live in `crates/lumia-plugin-api`.
- Process plugins communicate with the host over newline-delimited stdio JSON-RPC.
- Image payloads must be passed by path plus metadata, not base64 or JSON-inline pixel buffers.
- Plugin permissions must be declared in the manifest and enforced by the host before real filesystem or network access is implemented.
- Official bundled plugins must use the same manifest, permission, and JSON-RPC protocol as third-party plugins.

## Module Organization Rules

- Each module file must have a single clear responsibility. Do NOT put unrelated code into the same file.
- Production Rust modules must stay at or below 500 lines. Treat 300 lines as a review threshold and split by responsibility before adding substantial behavior.
- `lib.rs` files in library crates must contain ONLY `mod` declarations and `pub use` re-exports — no business logic.
- `main.rs` in the binary crate should be a thin skeleton: `mod` declarations, constants, `actions!` macro, and `main()` — no business logic.
- UI widget helpers (button factories, etc.) go in `widgets.rs`, NOT inline in render methods.
- Render methods for different UI areas (toolbar, viewer, settings panel, image info) go in separate files.
- i18n strings live in `i18n.rs`; add new `TextKey` variants and `tr()` match arms there.
- Settings persistence logic lives in `persistence.rs`.
- Theme/color palette logic lives in `palette.rs`.
- Utility/formatting functions live in `util.rs`.
- When adding a new settings group: add the variant to `SettingsGroup` in `lumia-core/settings.rs`, add navigation in `settings_ui.rs`, and put the content renderer in its own `settings_*.rs` module.
- When adding a new plugin capability: add the variant in `lumia-plugin-api/manifest.rs`, add params/result types in `lumia-plugin-api/messages.rs`.

## GPUI Guidance

- Prefer stable GPUI element IDs and avoid expensive allocations in `Render::render`.
- GPUI is sourced from the workspace's current Zed dependency set. For framework behavior, prefer the locked dependency source and local tutorial over external docs when they differ. Any dependency policy change or major upgrade must include an ADR with the reason, API impact, and verification result.
- The `actions!` macro must stay in `main.rs` (crate root). Action types are referenced from other modules via `crate::OpenFile` etc.
- GPUI trait imports ( `InteractiveElement`, `ParentElement`, `StatefulInteractiveElement`, `StyledImage`, etc.) must be explicitly listed in each module that uses them — they do not carry over from other modules.

## UI Component Library

- Lumia uses `gpui-component` as the shared UI component library for `crates/lumia-app`. Besides reading existing Lumia code, you may also reference the upstream documentation and examples at `https://github.com/longbridge/gpui-component`.
- Keep the direct `gpui`/`gpui_platform` dependencies in `Cargo.toml` using the same unpinned git URL shape as `gpui-component`; do NOT add `rev = ...` there. Cargo treats `git+url` and `git+url?rev=...` as different sources, which creates two incompatible `gpui` crates even when both resolve to the same commit.
- Pin the actual Zed/GPUI revision through the committed `Cargo.lock` instead. If the Zed revision needs to change, use `cargo update` and verify the whole workspace rather than editing dependency source or vendoring Zed.
- Keep `rust-toolchain.toml` aligned with the Rust version required by the locked Zed revision. Recent Zed GPUI commits use Rust APIs such as `slice_as_array` and `cold_path`, so older local stable toolchains may fail even when dependency source is correct.
- Initialize the component library in `bootstrap.rs` with `gpui_component::init(cx)` before creating application UI, and keep the root view wrapped in `gpui_component::Root`.
- Prefer `gpui_component` widgets for common controls such as buttons. Button helpers belong in `widgets.rs` and should return `AnyElement` when shared across render modules.
- When bridging `gpui_component::button::Button::on_click` into `LumiaApp`, use the callback-provided `&mut Window` directly and update app state through the stored `WeakEntity`; avoid `update_in` unless the code specifically requires GPUI to resolve the entity window.
- Raw GPUI `div()`-based controls are still acceptable for viewer-specific interactions, context menu rows, drag regions, or cases where `gpui-component` does not expose the needed mouse/keyboard semantics.
- Keep component styling aligned with `Palette`; do not hard-code colors in component wrappers unless the palette cannot express the state.

## Verification

Run these before handing off code:

```powershell
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
```

For UI changes, also run:

```powershell
cargo run -p lumia-app
```

For release builds:

```powershell
cargo build --release -p lumia-app
```

## Git

- Keep commits focused.
- Use the Angular/Conventional Commits format for all future commit messages: `<type>(<scope>): <subject>`.
- Allowed commit types are `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, and `revert`.
- Use an imperative, lower-case subject without a trailing period, for example `feat(plugin): add stdio handshake`.
- Use `!` before the colon for breaking changes, and include a `BREAKING CHANGE:` footer when needed.
- Do not commit generated build artifacts.
- Do not rewrite or discard user changes unless explicitly asked.
