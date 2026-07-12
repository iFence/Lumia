# Large Image Progressive Viewing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Open very large common raster images in the Lumia process with a bounded preview and on-demand high-resolution tiles.

**Architecture:** Preserve GPUI's path-backed fast path for normal images. Put checked geometry and pure-Rust decode/cache primitives in `lumia-core`, and keep background scheduling, cancellation, LRU state, and tiled composition in `lumia-app`.

**Tech Stack:** Rust 1.95, GPUI, `image` 0.25, already locked pure-Rust format codecs, `memmap2`, filesystem-backed BGRA8 cache.

---

### Task 1: Record the exception and add checked geometry

**Files:**
- Create: `docs/adr/0005-in-process-large-image-decoding.md`
- Create: `crates/lumia-core/src/image/large.rs`
- Modify: `crates/lumia-core/src/image.rs`
- Test: `crates/lumia-core/src/image/large.rs`

**Steps:**

1. Write failing tests for `LargeImagePolicy::requires_tiling`, checked RGBA bytes, tile levels,
   edge rectangles, and visible-tile intersection. Include 34752×11584.
2. Run `cargo test -p lumia-core image::large::tests`; expect missing types.
3. Implement pure checked geometry with 8192 safe edge, 256 MiB decoded threshold, and 512 tiles.
4. Write ADR 0005 describing the user-directed in-process, pure-Rust exception and its risks.
5. Run the focused tests; expect PASS.
6. Commit with `feat(core): add large image geometry`.

### Task 2: Add a safe disk-backed raster cache

**Files:**
- Create: `crates/lumia-core/src/image/large/cache.rs`
- Create: `crates/lumia-core/src/image/large/error.rs`
- Modify: `crates/lumia-core/Cargo.toml`
- Modify: `Cargo.toml`
- Test: `crates/lumia-core/src/image/large/cache.rs`

**Steps:**

1. Write failing tests for checked file length, cache key invalidation, `.part` lifecycle, row offsets,
   mapped read/write, insufficient-space errors, and cleanup.
2. Run the focused tests; expect missing cache API.
3. Declare the already locked `memmap2` dependency and implement a BGRA8 row-layout cache.
4. Ensure every offset and length uses checked arithmetic and cache writes are path-confined.
5. Run the focused tests; expect PASS.
6. Commit with `feat(core): add disk backed image cache`.

### Task 3: Decode bounded previews for common formats

**Files:**
- Create: `crates/lumia-core/src/image/large/decode.rs`
- Create: `crates/lumia-core/src/image/large/png.rs`
- Create: `crates/lumia-core/src/image/large/mapped.rs`
- Modify: `crates/lumia-core/Cargo.toml`
- Modify: `Cargo.toml`
- Test: `crates/lumia-core/src/image/large/decode.rs`

**Steps:**

1. Generate test fixtures for PNG, JPEG, WebP, BMP, TIFF, and GIF. Assert bounded output dimensions,
   BGRA length, alpha handling, GIF first-frame behavior, cancellation, and corrupt-input errors.
2. Add a generated wide PNG test that proves preview allocation is based on output dimensions, not source pixels.
3. Run focused tests; expect missing decoder API.
4. Use the locked `png` row API for bounded PNG previews. Use mapped output for codecs that require a
   complete destination, then sample the mapping into a bounded preview.
5. Enforce `image` decoder limits, checked destination sizes, disk-space checks, and cancellation boundaries.
6. Run `cargo test -p lumia-core image::large`; expect PASS.
7. Commit with `feat(core): decode bounded large image previews`.

### Task 4: Build and read high-resolution tiles with bounded concurrency

**Files:**
- Create: `crates/lumia-core/src/image/large/tiles.rs`
- Create: `crates/lumia-app/src/tile_cache.rs`
- Test: both modules

**Steps:**

1. Write failing tests for cache raster creation, edge tiles, level scaling, stable ordering, cancellation,
   worker-count calculation, priority, and a 256 MiB prepared-tile LRU.
2. Run focused tests; expect missing APIs.
3. Generate the BGRA8 backing raster sequentially where required, and read independent 512 tiles concurrently.
4. Use `std::thread::available_parallelism()` and existing GPUI/background primitives; cap workers at six
   and enforce an in-flight pixel budget.
5. Run focused tests; expect PASS.
6. Commit with `feat(image): add bounded large image tiles`.

### Task 5: Add app-side large-image session and scheduling

**Files:**
- Create: `crates/lumia-app/src/large_image.rs`
- Modify: `crates/lumia-app/src/main.rs`
- Modify: `crates/lumia-app/src/app.rs`
- Modify: `crates/lumia-app/src/load_state.rs`
- Modify: `crates/lumia-app/src/image_loading.rs`
- Test: `crates/lumia-app/src/large_image.rs`

**Steps:**

1. Write failing tests for threshold dispatch, preview-first state, visible-before-prefetch ordering,
   generation cancellation, stale-result rejection, and preview preservation after tile failure.
2. Run focused tests; expect missing session API.
3. Start large-image work only after metadata selects the policy. Run decoders on the background executor,
   apply results through generation checks, and request missing tiles after viewport changes.
4. Keep normal image, HEIF, and Photoshop paths unchanged.
5. Run focused tests; expect PASS.
6. Commit with `feat(app): schedule progressive large images`.

### Task 6: Composite preview and visible tiles in GPUI

**Files:**
- Create: `crates/lumia-app/src/large_image_render.rs`
- Modify: `crates/lumia-app/src/main.rs`
- Modify: `crates/lumia-app/src/render.rs`
- Modify: `crates/lumia-app/src/viewer_actions.rs`
- Modify: `crates/lumia-app/src/window_actions.rs`
- Test: `crates/lumia-app/src/large_image_render.rs`

**Steps:**

1. Write failing coordinate tests for fit, actual size, zoom, pan, edge tiles, and quarter-turn rotations.
2. Run focused tests; expect missing render geometry.
3. Keep `render.rs` as a branch point and put tiled composition in the dedicated module. Use stable IDs,
   clip to the viewer, retain preview beneath missing tiles, and avoid pixel copies during render.
4. Trigger tile scheduling from named zoom/pan/rotation handlers, never from render polling.
5. Run focused tests; expect PASS.
6. Commit with `feat(ui): render progressive image tiles`.

### Task 7: Clean caches and complete verification

**Files:**
- Modify: `crates/lumia-app/src/bootstrap.rs`
- Modify: `crates/lumia-app/src/util.rs`
- Modify: `crates/lumia-app/src/i18n.rs`
- Modify: `README.md`

**Steps:**

1. Add tests for stale `.part` cleanup and user-facing disk/memory/corruption messages.
2. Implement non-blocking startup cleanup and document large GIF first-frame behavior and disk requirements.
3. Run `cargo fmt --check`, `cargo check --workspace --all-targets`, and `cargo test --workspace`.
4. Run `cargo run -p lumia-app -- "C:\Users\yulei\Downloads\背景图_upscayl_16x_high-fidelity-4x.png"`.
5. Verify preview latency, progressive sharpness, responsive pan/zoom, bounded memory, temporary disk cleanup,
   and unchanged normal PNG/JPEG/WebP behavior.
6. Run `cargo build --release -p lumia-app` and commit with `feat(app): support very large raster images`.
