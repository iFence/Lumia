# Large Image Progressive Viewing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Open very large common raster images quickly with a bounded preview, then replace visible regions with on-demand high-resolution tiles without blocking or exhausting the Lumia process.

**Architecture:** Keep GPUI's existing path-backed rendering for normal images. Route images above checked dimension or decoded-memory thresholds through a bundled process plugin that produces a preview and batches of disk-backed tiles; the app owns viewport scheduling, cancellation, LRU state, and compositing.

**Tech Stack:** Rust 1.95, GPUI, `image` 0.25 and its format codecs, newline-delimited JSON-RPC, serde, bounded worker threads, filesystem cache.

---

### Task 1: Add checked large-image geometry to `lumia-core`

**Files:**
- Create: `crates/lumia-core/src/image/large.rs`
- Modify: `crates/lumia-core/src/image.rs`
- Test: `crates/lumia-core/src/image/large.rs`

**Step 1: Write failing tests**

Add tests for `LargeImagePolicy::requires_tiling`, checked RGBA byte calculation,
`TileLevel`, `TileCoordinate`, source rectangles at image edges, and the set of tiles intersecting a viewport.
Include the concrete 34752×11584 case and assert that it requires tiling.

**Step 2: Verify the tests fail**

Run: `cargo test -p lumia-core image::large::tests`
Expected: FAIL because `image::large` and its types do not exist.

**Step 3: Implement the minimal geometry API**

Define policy constants for 8192 maximum safe edge, 256 MiB decoded bytes, and 512-pixel tiles.
Use `checked_mul` for byte and tile counts. Add pure functions that map viewport coordinates and zoom level to clamped source rectangles and tile coordinates. Keep decoding and UI types out of this module.

**Step 4: Verify the tests pass**

Run: `cargo test -p lumia-core image::large::tests`
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/lumia-core/src/image.rs crates/lumia-core/src/image/large.rs
git commit -m "feat(core): add large image tile geometry"
```

### Task 2: Extend the plugin protocol for large-image batches

**Files:**
- Modify: `crates/lumia-plugin-api/src/manifest.rs`
- Modify: `crates/lumia-plugin-api/src/messages.rs`
- Modify: `crates/lumia-plugin-api/src/lib.rs`
- Test: `crates/lumia-plugin-api/src/messages.rs`

**Step 1: Write failing serialization tests**

Specify stable snake-case JSON for new `PluginCapability::DecodeTiles`, `LargeImageProbeResult`,
`DecodeLargePreviewParams`, `DecodeTileBatchParams`, `TileRequest`, `TileOutput`, and batch result types.
Requests contain paths and coordinates only; responses contain output paths and metadata only.

**Step 2: Verify the tests fail**

Run: `cargo test -p lumia-plugin-api`
Expected: FAIL because the capability and message types do not exist.

**Step 3: Implement and re-export protocol types**

Add the types without business logic. Increment `PROTOCOL_VERSION` only if compatibility tests show the
new capability cannot remain additive; otherwise retain version 1.

**Step 4: Verify the tests pass**

Run: `cargo test -p lumia-plugin-api`
Expected: PASS with exact JSON assertions.

**Step 5: Commit**

```powershell
git add crates/lumia-plugin-api
git commit -m "feat(plugin-api): add large image tile protocol"
```

### Task 3: Add host validation and batch calls

**Files:**
- Modify: `crates/lumia-plugin-host/src/process.rs`
- Modify: `crates/lumia-plugin-host/src/error.rs`
- Test: `crates/lumia-plugin-host/src/process.rs`

**Step 1: Write failing host tests**

Test that preview and tile calls reject missing capabilities, missing input/output permissions, empty batches,
rectangles outside declared dimensions, duplicate output paths, and output paths outside the supplied cache root.

**Step 2: Verify the tests fail**

Run: `cargo test -p lumia-plugin-host`
Expected: FAIL because large-image validation and methods are absent.

**Step 3: Add validated RPC methods**

Add `probe_large_image`, `decode_large_preview`, and `decode_tile_batch`. Reuse the persistent child process,
keep JSON payloads pixel-free, and map stable plugin error categories into `PluginHostError`.
Preserve the Windows `CREATE_NO_WINDOW` process flag already present in the working tree.

**Step 4: Verify the tests pass**

Run: `cargo test -p lumia-plugin-host`
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/lumia-plugin-host/src/process.rs crates/lumia-plugin-host/src/error.rs
git commit -m "feat(plugin-host): validate large image requests"
```

### Task 4: Scaffold the bundled large-image plugin

**Files:**
- Create: `plugins/lumia-plugin-large-image/Cargo.toml`
- Create: `plugins/lumia-plugin-large-image/src/main.rs`
- Create: `plugins/lumia-plugin-large-image/src/manifest.rs`
- Create: `plugins/lumia-plugin-large-image/src/server.rs`
- Create: `plugins/lumia-plugin-large-image/src/error.rs`
- Modify: `Cargo.toml`
- Test: `plugins/lumia-plugin-large-image/src/server.rs`

**Step 1: Write failing server tests**

Test initialization, capabilities, malformed JSON, unknown methods, permission-safe output validation, and
stable error kinds. The manifest must advertise common raster MIME inputs and PNG tile outputs.

**Step 2: Verify the new crate is not yet buildable**

Run: `cargo test -p lumia-plugin-large-image`
Expected: FAIL because the crate does not exist.

**Step 3: Implement the thin server skeleton**

Follow `lumia-plugin-photoshop`: keep `main.rs` as stdin/stdout setup, dispatch in `server.rs`, manifest creation
in `manifest.rs`, and error mapping in `error.rs`. Do not decode pixels yet.

**Step 4: Verify server tests pass**

Run: `cargo test -p lumia-plugin-large-image`
Expected: PASS.

**Step 5: Commit**

```powershell
git add Cargo.toml plugins/lumia-plugin-large-image
git commit -m "feat(plugin): scaffold large image worker"
```

### Task 5: Implement bounded preview decoding for common formats

**Files:**
- Create: `plugins/lumia-plugin-large-image/src/decode.rs`
- Create: `plugins/lumia-plugin-large-image/src/formats/mod.rs`
- Create: `plugins/lumia-plugin-large-image/src/formats/png.rs`
- Create: `plugins/lumia-plugin-large-image/src/formats/jpeg.rs`
- Create: `plugins/lumia-plugin-large-image/src/formats/raster.rs`
- Modify: `plugins/lumia-plugin-large-image/src/server.rs`
- Test: `plugins/lumia-plugin-large-image/src/decode.rs`

**Step 1: Add generated-format tests**

Generate small fixtures during tests for PNG, JPEG, WebP, BMP, TIFF, and GIF. Assert bounded dimensions,
RGBA output length, first-frame GIF behavior, atomic `.part` writes, cancellation, and resource-limit errors.
Add a sparse or generated 34752×11584 PNG fixture that does not require committing a large binary.

**Step 2: Verify tests fail**

Run: `cargo test -p lumia-plugin-large-image decode`
Expected: FAIL because no decoder exists.

**Step 3: Implement format-aware bounded decoding**

Use row-oriented PNG/BMP paths, decoder scaling for JPEG where available, TIFF strip/tile access where available,
and a bounded first-frame fallback for WebP/GIF. Decode directly toward the requested preview dimensions and
never allocate `source_width × source_height × 4` in the main process. Fail before allocation when a backend
cannot honor its memory budget.

**Step 4: Verify tests and measure the concrete case**

Run: `cargo test -p lumia-plugin-large-image decode`
Expected: PASS; the large PNG test stays within the configured pixel-buffer budget.

**Step 5: Commit**

```powershell
git add plugins/lumia-plugin-large-image
git commit -m "feat(plugin): decode bounded large image previews"
```

### Task 6: Implement tile production and bounded parallelism

**Files:**
- Create: `plugins/lumia-plugin-large-image/src/tiles.rs`
- Create: `plugins/lumia-plugin-large-image/src/worker_pool.rs`
- Create: `plugins/lumia-plugin-large-image/src/cache.rs`
- Modify: `plugins/lumia-plugin-large-image/src/server.rs`
- Test: `plugins/lumia-plugin-large-image/src/tiles.rs`
- Test: `plugins/lumia-plugin-large-image/src/worker_pool.rs`

**Step 1: Write failing tile and concurrency tests**

Test edge tiles, batch ordering, atomic output, cache reuse/invalidation, worker count
`max(1, min(logical_cores - 1, 6))`, priority ordering, cancellation, and a 256 MiB in-flight pixel semaphore.

**Step 2: Verify tests fail**

Run: `cargo test -p lumia-plugin-large-image tiles worker_pool`
Expected: FAIL.

**Step 3: Implement tiles and the bounded pool**

Process visible batches before prefetch. Parallelize independent resize/color/encode work, but keep sequential
format streams sequential. Key disk cache entries by canonical path, size, and modification time. Use `.part`
then rename and reject paths outside the cache root.

**Step 4: Verify tests pass**

Run: `cargo test -p lumia-plugin-large-image`
Expected: PASS without unbounded thread or buffer growth.

**Step 5: Commit**

```powershell
git add plugins/lumia-plugin-large-image
git commit -m "feat(plugin): generate large image tiles concurrently"
```

### Task 7: Add app-side session, scheduler, and LRU

**Files:**
- Create: `crates/lumia-app/src/large_image.rs`
- Create: `crates/lumia-app/src/tile_cache.rs`
- Modify: `crates/lumia-app/src/main.rs`
- Modify: `crates/lumia-app/src/app.rs`
- Modify: `crates/lumia-app/src/load_state.rs`
- Modify: `crates/lumia-app/src/image_loading.rs`
- Test: `crates/lumia-app/src/large_image.rs`
- Test: `crates/lumia-app/src/tile_cache.rs`

**Step 1: Write failing state tests**

Test threshold dispatch, preview-first transitions, visible-before-prefetch ordering, generation cancellation,
stale completion rejection, plugin failure preserving preview, and a 256 MiB prepared-tile LRU.

**Step 2: Verify tests fail**

Run: `cargo test -p lumia-app large_image tile_cache`
Expected: FAIL because session and cache modules do not exist.

**Step 3: Implement orchestration**

Start the bundled plugin only for images selected by `LargeImagePolicy`. Load the preview on GPUI's background
executor, calculate missing tiles after viewport changes, batch requests, and convert completed PNG outputs into
stable `PreparedImage` values. Keep original document metadata and discard stale generations.

**Step 4: Verify tests pass**

Run: `cargo test -p lumia-app large_image tile_cache`
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/lumia-app/src/main.rs crates/lumia-app/src/app.rs crates/lumia-app/src/load_state.rs crates/lumia-app/src/image_loading.rs crates/lumia-app/src/large_image.rs crates/lumia-app/src/tile_cache.rs
git commit -m "feat(app): schedule progressive large image loading"
```

### Task 8: Composite preview and visible tiles in GPUI

**Files:**
- Create: `crates/lumia-app/src/large_image_render.rs`
- Modify: `crates/lumia-app/src/main.rs`
- Modify: `crates/lumia-app/src/render.rs`
- Modify: `crates/lumia-app/src/viewer_actions.rs`
- Modify: `crates/lumia-app/src/window_actions.rs`
- Test: `crates/lumia-app/src/large_image_render.rs`

**Step 1: Write failing render-geometry tests**

Test original-to-display coordinate mapping for fit, actual size, zoom, pan, edge tiles, and quarter-turn rotations.
Assert that preview remains visible beneath missing tiles.

**Step 2: Verify tests fail**

Run: `cargo test -p lumia-app large_image_render`
Expected: FAIL.

**Step 3: Implement a dedicated renderer**

Keep `render.rs` as the branch point and place tiled composition in `large_image_render.rs`. Give each tile a
stable GPUI ID, position it from original-image coordinates, clip to the viewer, and request a scheduling refresh
after zoom/pan/rotation changes. Do not copy pixel buffers during `Render::render`.

**Step 4: Verify tests pass**

Run: `cargo test -p lumia-app large_image_render`
Expected: PASS.

**Step 5: Commit**

```powershell
git add crates/lumia-app/src/main.rs crates/lumia-app/src/render.rs crates/lumia-app/src/large_image_render.rs crates/lumia-app/src/viewer_actions.rs crates/lumia-app/src/window_actions.rs
git commit -m "feat(ui): render progressive large image tiles"
```

### Task 9: Package, clean cache, and complete integration verification

**Files:**
- Modify: `crates/lumia-app/build.rs`
- Modify: `crates/lumia-app/Cargo.toml`
- Modify: `crates/lumia-app/src/bootstrap.rs`
- Modify: `README.md`
- Modify: packaging files discovered by `rg -n "lumia-plugin-photoshop" .`
- Test: bundled development and release layouts

**Step 1: Add a failing packaging assertion**

Extend the existing bundled-plugin packaging test or build-time check so it requires
`lumia-plugin-large-image[.exe]` and its manifest beside the application.

**Step 2: Verify it fails**

Run the focused packaging test or `cargo build -p lumia-app`.
Expected: FAIL because the new plugin is not copied or declared.

**Step 3: Wire packaging and cache cleanup**

Mirror Photoshop plugin discovery and packaging. On startup, remove stale `.part` files and expired cache entries
without blocking the UI. Document supported large-image formats and first-frame behavior for huge GIF files.

**Step 4: Run the full automated verification**

Run:

```powershell
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo build --release -p lumia-app
```

Expected: all commands succeed.

**Step 5: Run interactive verification**

Run: `cargo run -p lumia-app -- "C:\Users\yulei\Downloads\背景图_upscayl_16x_high-fidelity-4x.png"`

Verify preview latency, progressive sharpness while zooming, responsive pan/navigation, no console flash, and
bounded Lumia/plugin memory in Task Manager. Also open normal PNG/JPEG/WebP files to confirm the fast path is unchanged.

**Step 6: Commit**

```powershell
git add crates/lumia-app README.md plugins/lumia-plugin-large-image
git commit -m "build(app): bundle large image plugin"
```
