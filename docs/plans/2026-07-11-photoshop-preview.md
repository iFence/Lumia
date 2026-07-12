# Photoshop PSD/PSB Preview Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Add safe, bundled PSD/PSB composite preview support without adding Photoshop decoding to Lumia's core or UI process.

**Architecture:** A new official process plugin probes and decodes PSD/PSB files to bounded temporary PNG previews through the existing newline-delimited JSON-RPC transport. `lumia-app` routes professional formats to the plugin while preserving the source Photoshop path in viewer state and reusing load generations and adjacent-image caching.

**Tech Stack:** Rust 2021 workspace, GPUI, serde JSON-RPC, `ag-psd`, `image`, path-based plugin I/O.

---

### Task 1: Harden the generic plugin contract

**Files:**
- Modify: `crates/lumia-plugin-api/src/messages.rs`
- Modify: `crates/lumia-plugin-api/src/rpc.rs`
- Modify: `crates/lumia-plugin-host/src/process.rs`
- Modify: `crates/lumia-plugin-host/src/error.rs`
- Test: inline unit tests in the files above

1. Add a typed `DecodePreviewResult` containing `ImageOutput`, decoded dimensions, and format metadata.
2. Add structured RPC error data with stable plugin error categories.
3. Write failing serialization and validation tests.
4. Add manifest/protocol validation after `plugin.initialize` and permission checks before path requests.
5. Run `cargo test -p lumia-plugin-api -p lumia-plugin-host` and expect all tests to pass.

### Task 2: Add the bundled Photoshop plugin shell and probe

**Files:**
- Create: `plugins/lumia-plugin-photoshop/Cargo.toml`
- Create: `plugins/lumia-plugin-photoshop/lumia.plugin.json`
- Create: `plugins/lumia-plugin-photoshop/src/main.rs`
- Create: `plugins/lumia-plugin-photoshop/src/header.rs`
- Create: `plugins/lumia-plugin-photoshop/src/error.rs`
- Modify: `Cargo.toml`

1. Add header tests for valid PSD version 1, valid PSB version 2, bad `8BPS` signature, invalid dimensions, and resource-limit rejection.
2. Run the focused test and verify it fails before implementation.
3. Implement bounded header parsing and `image.probe`.
4. Add the plugin manifest with `probe`, `decode_preview`, `read_input_path`, and `write_temporary_output`.
5. Run `cargo test -p lumia-plugin-photoshop` and expect probe tests to pass.

### Task 3: Decode a bounded composite preview

**Files:**
- Create: `plugins/lumia-plugin-photoshop/src/decode.rs`
- Modify: `plugins/lumia-plugin-photoshop/src/main.rs`
- Modify: `plugins/lumia-plugin-photoshop/Cargo.toml`
- Test: inline tests plus small generated fixtures where supported

1. Add failing tests for aspect-preserving bounds, output-parent validation, incomplete-output cleanup, and PSD/PSB composite conversion.
2. Parse through `ag-psd`, extract stored composite RGBA pixels, resize only when bounds require it, and atomically write PNG.
3. Map parser failures to stable error categories without leaking source paths.
4. Run `cargo test -p lumia-plugin-photoshop` and expect all tests to pass.

### Task 4: Add application plugin routing and cache state

**Files:**
- Create: `crates/lumia-app/src/plugin_catalog.rs`
- Create: `crates/lumia-app/src/professional_decode.rs`
- Modify: `crates/lumia-app/src/main.rs`
- Modify: `crates/lumia-app/src/app.rs`
- Modify: `crates/lumia-app/src/image_loading.rs`
- Modify: `crates/lumia-app/src/load_state.rs`
- Modify: `crates/lumia-app/Cargo.toml`

1. Add tests for executable-relative plugin discovery, exact plugin identity/capability validation, cache-key invalidation, and PSD/PSB routing.
2. Implement a narrow official-plugin catalog with no arbitrary directory scanning.
3. Run probe/decode in the background, apply only the current load generation, and decode the returned PNG through `lumia-core`.
4. Preserve the original source document path and metadata; queue adjacent professional previews one at a time.
5. Run `cargo test -p lumia-app` and expect routing/cache tests to pass.

### Task 5: Expose PSD/PSB throughout the product

**Files:**
- Modify: `crates/lumia-core/src/image/formats.rs`
- Modify: `crates/lumia-core/src/navigation.rs`
- Modify: `crates/lumia-app/src/settings_associations.rs`
- Modify: `crates/lumia-app/src/file_association_actions.rs`
- Modify: `crates/lumia-app/src/i18n.rs`
- Modify: `README.md`

1. Add a Photoshop format group containing `psd` and `psb`; update exact-coverage and navigation tests.
2. Include both extensions in file-open filters and OS association operations through the existing format-group data.
3. Add localized errors for missing plugin, unsupported Photoshop feature, corrupt document, and resource limit.
4. Document flattened-preview scope and compatibility limits.
5. Run `cargo test -p lumia-core -p lumia-app` and expect all tests to pass.

### Task 6: Full verification and packaging smoke test

**Files:**
- Modify only files required by verification findings

1. Run `cargo fmt --check`.
2. Run `cargo check --workspace --all-targets`.
3. Run `cargo test --workspace`.
4. Run `cargo run -p lumia-app` and manually open representative PSD/PSB and ordinary images.
5. Run `cargo build --release -p lumia-app` and confirm the Photoshop plugin has a documented colocated build artifact.
6. Review `git diff --check`, `git diff --stat`, and ensure no generated artifacts are tracked.
