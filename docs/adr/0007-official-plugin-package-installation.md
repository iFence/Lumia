# ADR 0007: Install only officially signed plugin packages

## Status

Accepted on 2026-07-28.

## Context

Lumia already discovers the optional Annotation plugin from fixed per-user
directories, but users must manually extract a release archive and copy its
directory. That makes the released plugin difficult to discover and use.

The current `lumia.plugin.sig` authenticates the exact runtime manifest bytes.
The manifest authenticates declared assets, but it does not authenticate the
platform executable produced by CI. This is sufficient for the current
manually managed, allowlisted prototype, but it is not a safe basis for an
application feature that installs downloaded executables.

The first installer must remain small, offline, cross-platform, and compatible
with Lumia's process-plugin boundary. It must not introduce a plugin store,
arbitrary third-party trust, background downloading, or plugin execution
during installation.

## Decision

- Add a **Plugins** group to Settings. Plugin installation and removal are
  application-management actions and do not appear in the image context menu.
- Accept only a platform-specific `.lumiaplugin` file using a ZIP container.
- Accept only plugin IDs on Lumia's official allowlist and packages signed by
  Lumia's embedded Ed25519 public key.
- Add `lumia.package.json` and `lumia.package.sig` at the archive root. The
  signed package manifest declares the package schema, plugin identity and
  version, plugin API compatibility, minimum Lumia version, target OS and
  architecture, install directory, and SHA-256 plus size for every payload
  file.
- Verify the signature over the exact `lumia.package.json` bytes. Do not
  reserialize JSON before verification.
- Retain `lumia.plugin.json` and `lumia.plugin.sig` for runtime discovery and
  protocol validation. Package verification and runtime verification are
  separate trust checks.
- Extract into a random staging directory below the per-user plugin root.
  Reject absolute paths, parent traversal, links, duplicate or
  case-colliding paths, undeclared files, and configured size/count limits.
- Install or update by atomically replacing the plugin directory. Preserve the
  previous version until the new directory is committed, and roll back on
  failure.
- Installation means enabled; removal means disabled. The first version has no
  enable switch, online catalog, automatic update, or third-party trust UI.
- Installation requires an application restart before contributions become
  active. Removal immediately removes the plugin from the in-memory registry
  and completes filesystem cleanup without leaving callable menu items.
- Release jobs generate and sign each platform package after compiling the
  executable. The signing private key is supplied through a GitHub Actions
  secret and must never be committed or printed.

## Consequences

### Positive

- Users can install Annotation without learning platform data directories.
- The complete executable package, not only its protocol manifest, is
  authenticated before Lumia writes it into an executable search location.
- Settings provides a durable home for future official plugins while the image
  context menu remains focused on image actions.
- Atomic replacement protects a working installed version from interrupted
  updates.

### Negative

- Release CI requires protected signing-key management.
- Lumia gains ZIP parsing and transactional filesystem-installation code.
- A package must be built and signed separately for each OS and architecture.
- The first version requires a restart after installation.

### Neutral

- Existing manually installed official plugin directories remain discoverable.
- Third-party plugins continue to use the shared protocol shapes, but Lumia
  does not install or load them in this phase.

## Alternatives Considered

**Put installation in the image context menu**

Rejected because package management is not an operation on the current image.
The context menu continues to show only contributions from installed plugins.

**Ship Annotation only as an application-installer option**

Rejected as the sole solution because it does not cover portable Windows,
macOS, Linux, later updates, or removal. It may be added later as a convenience.

**Trust only `lumia.plugin.sig`**

Rejected because the platform executable is not covered by that signature.

**Add an online plugin store**

Rejected for the first version because networking, catalog trust, download
recovery, and update policy add complexity unrelated to making the existing
official release artifact usable.

## References

- `docs/adr/0001-core-and-plugin-boundary.md`
- `docs/adr/0006-declarative-plugin-ui-and-annotation.md`
- `docs/plans/2026-07-28-official-plugin-installation-design.md`

