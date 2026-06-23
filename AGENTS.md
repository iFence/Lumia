# AGENTS.md

## Project Intent

Lumia is a cross-platform, minimal image viewer. The core app should stay small, fast, and maintainable. Broad image format support, image editing, compression, conversion, super-resolution, and cloud AI features belong behind the plugin boundary unless there is an explicit architecture decision to move them into core.

## Architecture Rules

- Use Rust and GPUI for the desktop application.
- Keep UI code in `crates/lumia-app` thin; put reusable viewer state and task models in `crates/lumia-core`.
- Do not add heavy decoder, AI, networking, or image-processing SDKs to `lumia-app`.
- All plugin-facing request and response shapes must live in `crates/lumia-plugin-api`.
- Process plugins communicate with the host over newline-delimited stdio JSON-RPC.
- Image payloads must be passed by path plus metadata, not base64 or JSON-inline pixel buffers.
- Plugin permissions must be declared in the manifest and enforced by the host before real filesystem or network access is implemented.

## GPUI Guidance

- Use `GPUI-Developer-Tutorial.md` as the local reference before introducing new UI patterns.
- Prefer stable GPUI element IDs and avoid expensive allocations in `Render::render`.
- GPUI is pinned in the workspace. Any upgrade must include an ADR with the reason, API impact, and verification result.

## Verification

Run these before handing off code:

```powershell
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
```

For UI changes, also run:

```powershell
cargo run -p lumia-app
```

## Git

- Keep commits focused.
- Use the Angular/Conventional Commits format for all future commit messages: `<type>(<scope>): <subject>`.
- Allowed commit types are `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, and `revert`.
- Use an imperative, lower-case subject without a trailing period, for example `feat(plugin): add stdio handshake`.
- Use `!` before the colon for breaking changes, and include a `BREAKING CHANGE:` footer when needed.
- Do not commit generated build artifacts.
- Do not rewrite or discard user changes unless explicitly asked.
