# ADR 0002: GPUI Version Policy

## Status

Accepted

## Context

Lumia uses GPUI for the desktop shell and `gpui-component` for shared UI controls. GPUI is pre-1.0 and currently consumed through the Zed dependency set used by `gpui-component`. Cargo treats `git+url` and `git+url?rev=...` as distinct sources, which can produce incompatible duplicate `gpui` crates even when both resolve to the same commit.

The workspace now depends on `gpui` and `gpui_platform` from `https://github.com/zed-industries/zed`, while `gpui-component` comes from `https://github.com/longbridge/gpui-component`. The actual resolved Zed revision is pinned by the committed `Cargo.lock`.

## Decision

- Keep direct `gpui` and `gpui_platform` dependencies using the same unpinned git URL shape as `gpui-component`.
- Pin the actual Zed/GPUI revision through `Cargo.lock`, not by adding `rev = ...` to the direct GPUI dependencies.
- Update GPUI through Cargo dependency resolution and commit the resulting `Cargo.lock` changes.
- Keep `rust-toolchain.toml` aligned with the Rust version required by the locked Zed revision.
- Record future GPUI policy changes or major upgrades in an ADR with the reason, API impact, and verification result.

## Consequences

- The workspace avoids duplicate incompatible `gpui` crates from mismatched git source URLs.
- Builds remain reproducible through the lockfile while allowing `gpui-component` and direct GPUI dependencies to share the same source identity.
- GPUI upgrades remain explicit and reviewable.
- Toolchain updates may be required when the locked Zed revision starts using newer Rust APIs.
