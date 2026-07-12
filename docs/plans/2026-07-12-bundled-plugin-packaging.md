# Bundled Plugin Packaging Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan task-by-task.

**Goal:** Ship the Photoshop preview plugin and manifest automatically in every Lumia release artifact.

**Architecture:** Release builds produce both binaries, then package the plugin under an executable-relative plugins/lumia-plugin-photoshop directory on every platform. WiX tracks plugin files as installer components; release workflows assert artifact contents before upload.

**Tech Stack:** Cargo workspace, cargo-wix/WiX v3, GitHub Actions, PowerShell, Bash, macOS app bundles.

---

### Task 1: Commit a maintained WiX installer definition

**Files:**
- Create: crates/lumia-app/wix/main.wxs
- Modify: .gitignore
- Modify: .github/workflows/release.yml

1. Add a maintained cargo-wix definition using the existing stable upgrade GUID.
2. Add WiX directories and components for the Photoshop executable and manifest.
3. Reference both components from the required application feature.
4. Build both release binaries before invoking cargo wix --no-build.
5. Build an MSI and verify its file table contains both plugin files.

### Task 2: Package portable and Unix bundles

**Files:**
- Modify: .github/workflows/release.yml
- Modify: crates/lumia-app/resources/install.sh
- Modify: crates/lumia-app/resources/Info.plist

1. Stage a Windows portable directory with the shared plugin layout before zipping.
2. Copy the plugin tree into the macOS app's Contents/MacOS directory.
3. Copy the plugin tree into the Linux release archive.
4. Install and uninstall the Linux plugin directory with the application.
5. Register PSD/PSB document types in the macOS bundle metadata.

### Task 3: Assert release artifact completeness

**Files:**
- Modify: .github/workflows/release.yml
- Modify: .github/workflows/ci.yml

1. Build both binaries in CI on all supported operating systems.
2. Assert the staged plugin executable and manifest exist before archive creation.
3. List archive contents and assert both expected plugin paths.
4. Keep upload steps configured to fail on missing artifacts.

### Task 4: Document and verify

**Files:**
- Modify: README.md

1. Document that official plugins are installed automatically.
2. Document the stable installed layout and development build command.
3. Run cargo fmt --check.
4. Run cargo check --workspace --all-targets --offline.
5. Run cargo test --workspace --offline.
6. Build both release binaries and the local MSI where WiX is available.
