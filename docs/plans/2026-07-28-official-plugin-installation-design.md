# Official Plugin Installation Design

## Status

Approved on 2026-07-28.

## Goal

Let a user install, update, reinstall, and remove the official Annotation
plugin from Lumia Settings without manually copying files, while authenticating
the complete executable package.

## Scope

The first version:

- installs a local `.lumiaplugin` file selected by the user;
- accepts only allowlisted Lumia official plugin IDs and the Lumia signing key;
- supports install, same-version reinstall, upgrade, and uninstall;
- blocks downgrade;
- requires restart after installation;
- remains fully offline.

It does not include a plugin store, downloads, automatic updates, arbitrary
third-party plugins, multiple trust roots, or an enable/disable switch.

## User Experience

Settings gains a **Plugins** group between File Associations and Shortcuts.
The page contains an **Install from file** button and one card per installed or
pending-restart plugin. A card shows name, official publisher badge, version,
short description, declared permissions, status, and reinstall/update/remove
actions.

Selecting a package opens a confirmation dialog showing current and target
versions, target platform, capabilities, and permissions. Verification and
installation run on a background executor. The viewer remains responsive.

Successful installation reports **Restart Lumia to enable this plugin**.
Successful removal hides the plugin's contributed commands immediately and
reports whether restart is required to finish cleanup. An empty installation
contributes no placeholder image-menu rows.

## Package Format

`.lumiaplugin` is a ZIP container:

```text
lumia.package.json
lumia.package.sig
lumia-plugin-annotation/
  lumia-plugin-annotation[.exe]
  lumia.plugin.json
  lumia.plugin.sig
  assets/
    pin.svg
    star.svg
    check.svg
```

Package manifest version 1 has this logical shape:

```json
{
  "schema_version": 1,
  "plugin_id": "lumia.annotation",
  "version": "0.1.0",
  "plugin_api_version": 2,
  "minimum_lumia_version": "0.1.2",
  "target_os": "windows",
  "target_arch": "x86_64",
  "install_directory": "lumia-plugin-annotation",
  "files": [
    {
      "path": "lumia-plugin-annotation/lumia-plugin-annotation.exe",
      "size": 123456,
      "sha256": "<64 lowercase hex characters>"
    }
  ]
}
```

The file list contains every payload file and excludes only
`lumia.package.json` and `lumia.package.sig`. The signature is Ed25519 over the
exact package-manifest bytes. The installer rejects undeclared or missing
files.

## Component Boundaries

- `settings_plugins.rs` renders plugin-management UI only.
- `plugin_package.rs` parses and verifies package metadata, signature, paths,
  limits, compatibility, and payload hashes.
- `plugin_installation.rs` owns platform roots, staging, atomic replacement,
  rollback, and removal.
- `plugin_catalog.rs` retains installed-plugin discovery and exposes the small
  mutations required to hide a removed plugin.
- `plugin_state.rs` remains responsible for active process sessions.
- `i18n.rs` owns all new user-facing strings.
- `scripts/sign-plugin-package.mjs` generates a deterministic package manifest
  and signature using Node's standard crypto APIs. Platform packaging scripts
  stage and ZIP the payload.

No installation logic enters `lumia-core`. Runtime protocol types remain in
`lumia-plugin-api`; package metadata may live there only if it is shared by
more than one Rust component.

## Installation Transaction

1. Reject a source file over the compressed-size limit.
2. Read the two root metadata files without extracting payload.
3. Verify package signature over exact manifest bytes.
4. Parse schema and require an allowlisted plugin ID.
5. Check target OS, architecture, plugin API, Lumia version, and downgrade
   policy.
6. Validate declared paths, counts, sizes, duplicates, and case collisions.
7. Extract regular files only into `plugins/.staging/<random-id>`.
8. Hash each staged file and compare its exact size and SHA-256.
9. Run existing runtime manifest-signature, asset, entry, and contribution
   validation against the staged directory.
10. Move an installed version to a same-filesystem backup.
11. Rename staging to `plugins/<install-directory>`.
12. Delete the backup. If commit fails, restore it before returning an error.

Temporary and backup paths always stay below the resolved per-user plugin root.
Startup may clean abandoned staging directories created by a terminated older
installation.

## Security Limits

Initial limits should be constants with tests:

- compressed package: 128 MiB;
- total uncompressed payload: 512 MiB;
- one file: 256 MiB;
- files: 512;
- path UTF-8 bytes: 240;
- metadata files: 1 MiB each.

Reject encrypted ZIP entries, links, non-regular file types, absolute/rooted
paths, `..`, Windows device names, trailing dots/spaces, NUL, duplicate paths,
and case-insensitive collisions. Never execute the plugin during installation.
Do not expose signing secrets or raw secret-bearing environment values in logs.

## Failure Handling

Stable UI categories are: invalid package, unofficial signature, damaged
payload, incompatible platform, incompatible version, downgrade blocked,
insufficient space, permission denied, plugin busy, and replacement failed.
Technical chains remain available in diagnostic logging.

Failure before commit deletes staging. Failure during replacement restores the
backup. A failed install must not change the active registry or current plugin.

## Non-Functional Requirements

- Installation work runs off the GPUI thread and reports bounded progress.
- Normal startup does not inspect archives or perform network requests.
- Absence of optional plugins preserves current startup and viewer behavior.
- The trust root is compiled into Lumia; the private key exists only in
  protected release automation.
- Filesystem operations are per-user and require no administrator permission.
- All production Rust modules remain at or below 500 lines.

## Verification

Automated coverage includes package parsing, exact-byte signature verification,
tampered executable and asset rejection, every unsafe path class, archive
limits, wrong target/API/version, downgrade rejection, same-version reinstall,
upgrade, rollback, removal, registry visibility, and settings-state rendering.

Release jobs verify the final `.lumiaplugin` with the same production verifier
before upload. Manual acceptance installs Annotation from a release asset,
restarts Lumia, opens an image, invokes **Annotate / 标注**, removes the plugin,
and confirms the command is no longer available.

