# AGENTS.md

## Project Intent

Lumia is a cross-platform, minimal image viewer. The core app should stay small, fast, and maintainable. Broad image format support, image editing, compression, conversion, super-resolution, and cloud AI features belong behind the plugin boundary unless there is an explicit architecture decision to move them into core.

## Workspace Structure

```
crates/
  lumia-core/src/          -- 领域模型库（无 UI 依赖）
    lib.rs                 -- 仅 re-export，不包含业务逻辑
    image.rs               -- ImageDocument, ImageLoadError, ImageMetadata, ColorDescription, PixelFormat, TransferFunction, 扩展名工具函数
    viewport.rs            -- ViewportState, FitMode
    task.rs                -- TaskState, TaskStatus
    settings.rs            -- AppSettings, Language, ThemeMode, SettingsGroup

  lumia-plugin-api/src/    -- 插件协议类型库（纯数据，无运行时逻辑）
    lib.rs                 -- 仅 re-export
    rpc.rs                 -- JSON_RPC_VERSION, PROTOCOL_VERSION, RpcId, JsonRpcRequest, JsonRpcResponse, RpcError
    manifest.rs            -- PluginManifest, PluginCapability, PluginPermission
    messages.rs            -- 所有 RPC 参数/结果类型, ImageOperation, ImagePath, ImageOutput

  lumia-plugin-host/src/   -- 插件进程管理器（子进程生成 + stdio JSON-RPC 传输）
    lib.rs                 -- PluginProcess, PluginHostError

  lumia-app/src/           -- GPUI 桌面应用（二进制 crate）
    main.rs                -- 入口点：mod 声明、常量、actions! 宏、main()（约 35 行，不包含业务逻辑）
    app.rs                 -- LumiaApp 结构体 + 全部事件处理器 + 图像加载 + 缩放计算 + Focusable impl
    render.rs              -- Render trait 实现 + render_toolbar + render_viewer + render_context_menu
    settings_ui.rs         -- 设置面板渲染：render_settings_panel 等 5 个方法
    image_info.rs          -- 图像信息遮罩：render_image_info_overlay + image_info_lines
    widgets.rs             -- UI 组件工厂函数：toolbar_button, context_menu_item, settings_group_button 等
    palette.rs             -- Palette 结构体 + theme_resolves_to_dark + impl LumiaApp { palette() }
    i18n.rs                -- TextKey 枚举 + tr() 翻译函数
    persistence.rs         -- load_settings, save_settings, settings_path, platform_config_dir
    util.rs                -- status_message, format_file_size, format_modified_time, format_load_error

plugins/
  lumia-plugin-sample/     -- 示例插件（最小 stdin/stdout JSON-RPC 循环）
```

## Crate Dependency Graph

```
lumia-app ──────> lumia-core
    │                  (无工作区依赖)
    └──────> lumia-plugin-host ──> lumia-plugin-api
                                            (无工作区依赖)

lumia-plugin-sample ──> lumia-plugin-api
```

- 无循环依赖
- `lumia-core` 和 `lumia-plugin-api` 是叶子 crate
- `lumia-app` 是唯一的整合点

## Architecture Rules

- Use Rust and GPUI for the desktop application.
- Keep UI code in `crates/lumia-app` thin; put reusable viewer state and task models in `crates/lumia-core`.
- Do not add heavy decoder, AI, networking, or image-processing SDKs to `lumia-app`.
- All plugin-facing request and response shapes must live in `crates/lumia-plugin-api`.
- Process plugins communicate with the host over newline-delimited stdio JSON-RPC.
- Image payloads must be passed by path plus metadata, not base64 or JSON-inline pixel buffers.
- Plugin permissions must be declared in the manifest and enforced by the host before real filesystem or network access is implemented.

## Module Organization Rules

- Each module file must have a single clear responsibility. Do NOT put unrelated code into the same file.
- `lib.rs` files in library crates must contain ONLY `mod` declarations and `pub use` re-exports — no business logic.
- `main.rs` in the binary crate should be a thin skeleton: `mod` declarations, constants, `actions!` macro, and `main()` — no business logic.
- UI widget helpers (button factories, etc.) go in `widgets.rs`, NOT inline in render methods.
- Render methods for different UI areas (toolbar, viewer, settings panel, image info) go in separate files.
- i18n strings live in `i18n.rs`; add new `TextKey` variants and `tr()` match arms there.
- Settings persistence logic lives in `persistence.rs`.
- Theme/color palette logic lives in `palette.rs`.
- Utility/formatting functions live in `util.rs`.
- When adding a new settings group: add the variant to `SettingsGroup` in `lumia-core/settings.rs`, add sidebar + content renderers in `settings_ui.rs`.
- When adding a new plugin capability: add the variant in `lumia-plugin-api/manifest.rs`, add params/result types in `lumia-plugin-api/messages.rs`.

## GPUI Guidance

- Use `GPUI-Developer-Tutorial.md` as the local reference before introducing new UI patterns.
- Prefer stable GPUI element IDs and avoid expensive allocations in `Render::render`.
- GPUI is pinned in the workspace. Any upgrade must include an ADR with the reason, API impact, and verification result.
- The `actions!` macro must stay in `main.rs` (crate root). Action types are referenced from other modules via `crate::OpenFile` etc.
- GPUI trait imports ( `InteractiveElement`, `ParentElement`, `StatefulInteractiveElement`, `StyledImage`, etc.) must be explicitly listed in each module that uses them — they do not carry over from other modules.

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

For release builds:

```powershell
cargo build --release -p lumia-app
```

## Git

- Keep commits focused.
- Use the Angular/Conventional Commits format for all future commit messages: `<type>(<scope>): <subject>`.
- Allowed commit types are `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, and `revert`.
- Use an imperative, lower-case subject without a trailing period, for example `feat(plugin): add stdio handshake`.
- Use `!` before the colon for breaking changes, and include a `BREAKING CHANGE:` footer when needed.
- Do not commit generated build artifacts.
- Do not rewrite or discard user changes unless explicitly asked.
