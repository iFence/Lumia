# Official Plugin Installation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Add a safe Settings-based installer for platform-specific, officially signed `.lumiaplugin` packages.

**Architecture:** Lumia verifies a signed package manifest that hashes every payload file, extracts into a bounded staging directory, and atomically replaces the per-user installed plugin. Settings owns presentation, while package verification, filesystem transactions, runtime discovery, and release signing remain separate modules.

**Tech Stack:** Rust 2021, GPUI, gpui-component, serde/serde_json, ring Ed25519, SHA-256, ZIP, Node.js standard crypto, PowerShell, Bash, GitHub Actions.

---

### Task 1: Define package metadata and compatibility rules

**Files:**
- Create: `crates/lumia-app/src/plugin_package.rs`
- Modify: `crates/lumia-app/src/main.rs`
- Modify: `crates/lumia-app/Cargo.toml`
- Test: `crates/lumia-app/src/plugin_package.rs`

1. Add failing serde round-trip tests for schema version 1, file records, target
   OS/architecture, plugin API version, minimum Lumia version, and install
   directory.
2. Run `cargo test -p lumia-app plugin_package::tests -- --nocapture` and
   expect missing-type failures.
3. Add private `PluginPackageManifest` and `PluginPackageFile` types. Keep these
   app-local until another Rust crate needs them.
4. Add `zip` and `semver` dependencies using versions compatible with the
   workspace toolchain.
5. Add compatibility functions that return typed error variants for schema,
   allowlist, OS, architecture, API, app version, and downgrade failures.
6. Run the focused tests and expect them to pass.
7. Commit with `feat(plugin): define official package metadata`.

### Task 2: Verify signed archives without extracting untrusted paths

**Files:**
- Modify: `crates/lumia-app/src/plugin_package.rs`
- Modify: `crates/lumia-app/src/plugin_catalog.rs`
- Test: `crates/lumia-app/src/plugin_package.rs`

1. Add test helpers that create ZIP fixtures with a checked-in test keypair
   dedicated to tests; never use the production private key.
2. Write failing tests for a valid package, changed manifest bytes, tampered
   executable, missing/extra files, duplicate paths, case collisions,
   traversal, absolute paths, links, encrypted entries, and every size/count
   limit.
3. Run the focused tests and confirm each new case fails for the expected
   reason.
4. Move the embedded official public key and reusable exact-byte signature
   verifier into a small shared section of `plugin_package.rs`; update
   `plugin_catalog.rs` to call it without changing runtime behavior.
5. Implement metadata-first ZIP inspection and path normalization. Do not write
   payload files during this validation phase.
6. Stream each entry through SHA-256 and enforce declared size while reading;
   do not allocate the full archive or large files in memory.
7. Run focused tests, `cargo test -p lumia-app plugin_catalog`, and
   `cargo check -p lumia-app`.
8. Commit with `feat(plugin): verify signed plugin archives`.

### Task 3: Implement transactional installation and removal

**Files:**
- Create: `crates/lumia-app/src/plugin_installation.rs`
- Modify: `crates/lumia-app/src/main.rs`
- Modify: `crates/lumia-app/src/plugin_catalog.rs`
- Test: `crates/lumia-app/src/plugin_installation.rs`

1. Write failing tempfile-based tests for new install, same-version reinstall,
   upgrade, downgrade rejection, backup restoration, abandoned staging cleanup,
   uninstall, and attempts to escape the plugin root.
2. Run `cargo test -p lumia-app plugin_installation::tests -- --nocapture` and
   verify the tests fail before implementation.
3. Implement platform plugin-root resolution once and reuse it from discovery
   and installation.
4. Extract verified regular files to `.staging/<random-id>` using create-new
   semantics, then rerun hashes and existing runtime manifest/asset/entry
   validation against the staged tree.
5. Implement same-filesystem backup, rename, rollback, and cleanup. Return a
   typed result containing plugin ID, previous version, installed version, and
   restart requirement.
6. Add `PluginRegistry::remove(id)` so successful removal cannot leave a
   callable context-menu contribution in memory.
7. Run focused tests twice to catch cleanup/state leakage.
8. Commit with `feat(plugin): install packages transactionally`.

### Task 4: Add plugin-management application state

**Files:**
- Create: `crates/lumia-app/src/plugin_management.rs`
- Modify: `crates/lumia-app/src/main.rs`
- Modify: `crates/lumia-app/src/app.rs`
- Modify: `crates/lumia-app/src/plugin_state.rs`
- Test: `crates/lumia-app/src/plugin_management.rs`

1. Define tests for idle, selecting, confirming, verifying, installing,
   installed-pending-restart, removing, success, and categorized-error states.
2. Add `PluginManagementState` separately from active `PluginUiState`.
3. Implement background-executor commands for inspect, confirm/install, and
   remove. Use generation IDs so stale asynchronous results cannot overwrite a
   newer operation.
4. Block removal while the same plugin has an active session. Never force-kill
   it from Settings.
5. Refresh installed inventory after filesystem commits; installation remains
   pending restart, while removal deletes the registry entry immediately.
6. Run focused tests and `cargo check -p lumia-app`.
7. Commit with `feat(plugin): manage installation state`.

### Task 5: Build the Plugins settings page

**Files:**
- Modify: `crates/lumia-core/src/settings.rs`
- Modify: `crates/lumia-app/src/settings_ui.rs`
- Create: `crates/lumia-app/src/settings_plugins.rs`
- Modify: `crates/lumia-app/src/main.rs`
- Modify: `crates/lumia-app/src/i18n.rs`
- Modify: `crates/lumia-app/src/widgets.rs`
- Test: `crates/lumia-app/tests/architecture.rs`

1. Add `SettingsGroup::Plugins` and update exhaustive settings navigation tests.
2. Add failing translation-coverage and stable-element-ID tests for all plugin
   UI states.
3. Add the sidebar entry between File Associations and Shortcuts and dispatch
   its content to `settings_plugins.rs`.
4. Render the empty state, installed cards, official badge, version,
   permissions, install/update/reinstall/remove actions, confirmation dialog,
   busy state, restart notice, and categorized errors.
5. Use a `.lumiaplugin` file picker and existing gpui-component button helpers.
   Keep reusable button construction in `widgets.rs`.
6. Run `cargo test -p lumia-app`, launch `cargo run -p lumia-app`, and manually
   inspect light/dark themes plus English/Chinese text.
7. Commit with `feat(settings): add official plugin management`.

### Task 6: Generate and sign release packages

**Files:**
- Create: `scripts/sign-plugin-package.mjs`
- Modify: `scripts/package-annotation-plugin.ps1`
- Modify: `scripts/package-annotation-plugin.sh`
- Create: `scripts/verify-plugin-package.ps1`
- Modify: `.github/workflows/release.yml`
- Modify: `.github/workflows/ci.yml`

1. Add script fixture tests proving deterministic manifest ordering and
   signature verification with a test key.
2. Implement a Node standard-library signer that walks only the staged payload,
   rejects links, hashes every file, writes deterministic JSON, and signs its
   exact bytes from a protected environment variable.
3. Never print the private key or its environment variable. Fail release jobs
   when the signing secret is missing.
4. Update every platform packager to produce a ZIP container named
   `Lumia-Annotation-<os>-<arch>.lumiaplugin`.
5. Add a verification command that invokes Lumia's production package verifier
   against the final archive before upload.
6. Upload only verified `.lumiaplugin` artifacts and retain
   `fail_on_unmatched_files: true`.
7. Run local packaging with the test key, then verify tampering causes a
   non-zero exit.
8. Commit with `build(plugin): sign official plugin packages`.

### Task 7: Document installation and complete verification

**Files:**
- Modify: `README.md`
- Modify: `docs/adr/0006-declarative-plugin-ui-and-annotation.md`

1. Replace manual-copy instructions with Settings-based installation while
   retaining platform directories as troubleshooting information.
2. Link ADR 0006 to ADR 0007 and clarify that package signatures now cover the
   platform executable.
3. Run `cargo fmt --check`.
4. Run `cargo check --workspace --all-targets`.
5. Run `cargo test --workspace`.
6. Build release packages on each supported OS and install the resulting asset
   into a clean Lumia profile.
7. Confirm Annotation appears after restart, can annotate/export a copy, and
   disappears after removal.
8. Commit with `docs(plugin): document signed package installation`.

