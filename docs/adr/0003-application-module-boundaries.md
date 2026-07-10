# ADR 0003: Application module and state boundaries

## Status

Accepted

## Context

The GPUI application accumulated image loading, navigation, background decode,
window behavior, preferences, shortcuts, and multiple render regions in a few
large files. This made state transitions difficult to test and allowed render
methods to own background-task polling.

## Decision

- `LumiaApp` remains the GPUI integration entity, but reusable viewer and folder
  navigation state lives in `lumia-core`.
- Image loading owns explicit current-load and catalog generations. Background
  results are applied only when their generation remains current.
- Render modules compose elements and forward events to named handlers. They do
  not poll channels or own background task lifecycle.
- UI areas, settings domains, platform shell integrations, and image codec
  responsibilities use separate modules.
- Production Rust modules have a 500-line hard limit enforced by a workspace
  architecture test. Files approaching 300 lines should be reviewed for a
  natural responsibility split before more behavior is added.
- The in-core HEIC decoder remains an isolated compatibility bridge. Moving it
  to an official plugin requires a separate ADR and migration.

## Consequences

State transitions can be unit tested without GPUI, stale decode results cannot
replace a newer image, and future work has explicit ownership boundaries. The
trade-off is a larger number of small modules and some crate-private forwarding
methods on `LumiaApp`.
