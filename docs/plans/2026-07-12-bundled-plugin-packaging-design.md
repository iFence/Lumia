# Bundled Plugin Packaging Design

## Status

Approved on 2026-07-12.

## Goal

Official Lumia plugins must be installed, upgraded, and removed with Lumia. End users must not download or copy the Photoshop preview executable manually.

## Bundle Layout

All platforms use the same application-relative layout:

    <application executable directory>/
      lumia-app[.exe]
      plugins/
        lumia-plugin-photoshop/
          lumia-plugin-photoshop[.exe]
          lumia.plugin.json

This matches the existing executable-relative plugin resolver. The manifest is shipped beside the executable even though the current official-plugin catalog is embedded in the app, making the installed boundary inspectable and ready for a future manifest-driven catalog.

## Platform Packaging

- Windows MSI installs the application, plugin executable, and manifest as separate WiX components so upgrade and uninstall track every file.
- Windows portable ZIP preserves the same directory tree.
- macOS places the plugin under Lumia.app/Contents/MacOS/plugins/lumia-plugin-photoshop.
- Linux archives contain the plugin tree; install.sh copies it below ~/.local/bin/plugins and removes it on uninstall.

Release jobs build lumia-app and lumia-plugin-photoshop together. Packaging steps fail if either binary or the manifest is missing. Archive-content checks guard against regressions where a release succeeds but silently omits PSD/PSB support.

## Security and Failure Handling

Only the known official plugin is bundled. Packaging does not add arbitrary plugin scanning or installation of untrusted executables. The shipped manifest declares path permissions and no network access. A missing plugin remains a recoverable preview error, while release verification prevents official artifacts from being published in that state.

## Alternatives

- Installing both executables beside each other is simpler, but a dedicated plugin directory avoids collisions as bundled plugins grow.
- Downloading the plugin on first use adds networking, integrity verification, and offline failure modes, so it is rejected for the official default capability.
- Linking the decoder into the app would violate the process isolation ADR.
