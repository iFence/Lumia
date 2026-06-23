# ADR 0002: GPUI Version Policy

## Status

Accepted

## Context

Lumia uses GPUI for the desktop shell. GPUI is pre-1.0 and its APIs can change quickly. The local Cargo registry cache confirms `gpui = 0.2.2` is available.

## Decision

Pin GPUI to `=0.2.2` in the workspace. Do not depend directly on the Zed git repository for normal development.

## Consequences

- Builds are more reproducible.
- Upgrades are explicit and reviewable.
- When GPUI is upgraded, update this workspace with a new ADR describing the reason, code impact, and verification results on Windows, macOS, and Linux.
