# GPUI 开发者教程

> **基于 GPUI v0.2.2（Zed 编辑器 UI 框架）**
>
> 本教程基于对 [zed-industries/zed](https://github.com/zed-industries/zed) 源码的深入调研编写。所有代码示例均来自官方示例或源码中的真实 API。

---

## 目录

- [一、GPUI 简介](#一gpui-简介)
- [二、环境搭建](#二环境搭建)
- [三、核心概念](#三核心概念)
- [四、Hello World](#四hello-world)
- [五、布局系统](#五布局系统)
- [六、内置元素详解](#六内置元素详解)
- [七、事件处理](#七事件处理)
- [八、状态管理](#八状态管理)
- [九、样式和主题](#九样式和主题)
- [十、高级主题](#十高级主题)
- [十一、实战：图片查看器](#十一实战图片查看器)
- [十二、插件系统设计](#十二插件系统设计)
- [十三、性能优化](#十三性能优化)
- [十四、与 Qt/Tauri/Slint/Iced 对比](#十四与-tautaurislinticed-对比)
- [十五、资源和社区](#十五资源和社区)

---

## 一、GPUI 简介

### 1.1 是什么

GPUI 是一个**混合即时/保留模式（Hybrid Immediate + Retained Mode）、GPU 加速**的 Rust UI 框架，由 Zed 编辑器团队开发并开源。它是 Zed 编辑器的渲染引擎，专门设计用于构建**高性能桌面应用**。

**核心特性：**
- **GPU 加速渲染**：macOS 使用 Metal，Windows/Linux 使用原生 GPU 后端
- **声明式 UI**：类似 React 的 `Render` trait，状态变化时自动重绘
- **Tailwind 风格的样式 API**：链式调用 `.flex().items_center().bg(rgb(0x505050))`
- **Flexbox + Grid 布局**：基于 [Taffy](https://github.com/DioxusLabs/taffy) 布局引擎
- **跨平台**：macOS、Windows、Linux（Wayland/X11）、Web（WASM）
- **无障碍支持**：通过 AccessKit 集成
- **内置异步运行时**：与平台事件循环集成的 Tokio 风格异步执行器

### 1.2 不是什么

- ❌ **不是 Flutter/Tauri 那样的跨平台移动端框架** — 专注于桌面
- ❌ **不是 DOM/HTML 框架** — 没有 CSS，没有 HTML
- ❌ **不是纯即时模式 UI** — 结合了保留模式的优势（视图缓存、状态管理）
- ❌ **不是开箱即用的组件库** — 提供的是基础构建块（`div`、`text`、`img`），高级组件需要自行构建

### 1.3 设计哲学

GPUI 的设计哲学可以用三个词概括：

1. **性能优先** — 每帧都在重绘，但通过 GPU 加速和智能缓存实现 60fps+
2. **Rust 原生** — 没有 GC，没有 runtime，充分利用 Rust 的所有权系统
3. **Web 开发者友好** — Tailwind 样式 API、Flexbox 布局模型，降低 Web 开发者的学习曲线

### 1.4 适合什么场景

| ✅ 适合 | ❌ 不适合 |
|---------|-----------|
| 高性能桌面编辑器 | 移动端应用 |
| 开发者工具 | 简单表单应用（杀鸡用牛刀） |
| 实时数据可视化 | 需要丰富现成组件的应用 |
| 代码编辑器/IDE | 游戏 UI（考虑 egui/bevy_ui） |
| 自定义桌面 shell | 需要 100% 原生外观的应用 |

---

## 二、环境搭建

### 2.1 Rust 工具链

GPUI 要求**最新稳定版 Rust**：

```bash
# 安装 Rust（如果还没有）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 确保是最新稳定版
rustup update stable
```

### 2.2 平台特定依赖

#### macOS

GPUI 在 macOS 上使用 Metal 渲染，需要 Xcode：

```bash
# 安装 Xcode（从 App Store 或 Apple Developer 网站）
# 安装后启动一次，确保安装了 macOS 组件

# 安装 Xcode 命令行工具
xcode-select --install

# 确保指向正确的 Xcode
sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
```

#### Linux

需要 Wayland 或 X11 开发库：

```bash
# Ubuntu/Debian
sudo apt install pkg-config libfontconfig-dev libwayland-dev libxkbcommon-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-x11-dev libxcb-randr0-dev \
  libxcb1-dev libxrandr-dev

# Arch
sudo pacman -S pkg-config fontconfig wayland libxkbcommon libxcb
```

#### Windows

无需额外依赖。GPUI 在 Windows 上使用 Win32 窗口和 DirectWrite 文本渲染。

### 2.3 Cargo.toml 配置

GPUI 已发布到 crates.io，可以直接引用：

```toml
[dependencies]
gpui = "0.2"
gpui_platform = { version = "0.1", features = ["font-kit", "wayland", "x11"] }
```

**平台特性说明：**

| 平台 | 推荐配置 |
|------|---------|
| macOS | `gpui_platform = { version = "0.1", features = ["font-kit"] }` |
| Linux | `gpui_platform = { version = "0.1", features = ["wayland", "x11"] }` |
| Windows | `gpui_platform = { version = "0.1" }`（无需特性） |
| 跨平台默认 | `gpui_platform = { version = "0.1", features = ["font-kit", "wayland", "x11"] }` |

> **注意**：如果不启用 `font-kit`，macOS 上的文本会布局但不渲染字形（会显示占位符）。Linux 需要至少启用一个窗口后端（`wayland` 或 `x11` 或两者都启用）。

### 2.4 从 Git 引用（开发版）

如果你想使用最新未发布版本：

```toml
[dependencies]
gpui = { git = "https://github.com/zed-industries/zed" }
gpui_platform = { git = "https://github.com/zed-industries/zed" }
```

### 2.5 完整的最小 Cargo.toml

```toml
[package]
name = "my-gpui-app"
version = "0.1.0"
edition = "2021"

[dependencies]
gpui = "0.2"
gpui_platform = { version = "0.1", features = ["font-kit", "wayland", "x11"] }
anyhow = "1"
```

---

## 三、核心概念

GPUI 的核心概念是理解整个框架的关键。以下是它们的关系图：

```
┌─────────────────────────────────────────────────────┐
│                    Application                       │
│  ┌───────────────────────────────────────────────┐  │
│  │                    App (cx)                    │  │
│  │  • 所有 Entity 的拥有者                         │  │
│  │  • 全局状态 (Global)                            │  │
│  │  • 键绑定 (KeyBinding)                          │  │
│  │  • HTTP 客户端、异步执行器                       │  │
│  │                                                │  │
│  │  ┌──────────────┐    ┌──────────────┐         │  │
│  │  │   Window 1    │    │   Window 2    │        │  │
│  │  │ ┌──────────┐ │    │ ┌──────────┐ │        │  │
│  │  │ │Root View │ │    │ │Root View │ │        │  │
│  │  │ │(Entity)  │ │    │ │(Entity)  │ │        │  │
│  │  │ │          │ │    │ │          │ │        │  │
│  │  │ │ render() │ │    │ │ render() │ │        │  │
│  │  │ │    ↓     │ │    │ │    ↓     │ │        │  │
│  │  │ │ Element  │ │    │ │ Element  │ │        │  │
│  │  │ │  Tree    │ │    │ │  Tree    │ │        │  │
│  │  │ └──────────┘ │    │ └──────────┘ │        │  │
│  │  └──────────────┘    └──────────────┘         │  │
│  │                                                │  │
│  │  Entity<T> ──────┐                             │  │
│  │  Entity<T> ──────┤  被 Context<T> 访问         │  │
│  │  Entity<T> ──────┘  被 Window 观察              │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

### 3.1 App 和 Context

**`App`** 是整个应用的根上下文，拥有所有实体（Entity）的数据。它提供了：
- 创建和管理实体
- 全局状态存储（`Global` trait）
- 键绑定注册
- 异步执行器访问
- HTTP 客户端
- 系统操作（退出、打开 URL 等）

**`Context<T>`** 是特定实体的上下文，可以解引用到 `App`。当你的代码在处理一个特定实体时，`Context<T>` 提供了：
- 通知观察者（`cx.notify()`）
- 发射事件（`cx.emit()`）
- 订阅其他实体的事件
- 访问当前实体的句柄

```rust
// Context<T> 的核心方法
impl<T: 'static> Context<T> {
    // 获取当前实体句柄
    pub fn entity(&self) -> Entity<T>;
    
    // 通知所有观察者重绘
    // 调用后，观察此实体的 Window 会在下一帧重新调用 render()
    pub fn notify(&mut self);
    
    // 向订阅者发射事件
    pub fn emit<E: 'static>(&mut self, event: E);
    
    // 观察其他实体的变化
    pub fn observe<W>(&mut self, entity: &Entity<W>, ...) -> Subscription;
    
    // 订阅其他实体的事件
    pub fn subscribe<T2, Evt>(&mut self, entity: &Entity<T2>, ...) -> Subscription;
}
```

**关系图：**

```
Application::run(|cx: &mut App| {
    // cx 是 App 类型 — 全局上下文
    
    cx.open_window(..., |_, cx| {
        // cx 是 Context<T> 类型 — 特定于视图
        // Context<T> 可以做 App 能做的一切（通过 Deref）
        // 加上额外的实体级操作
    });
});
```

### 3.2 Element trait

`Element` 是 GPUI 渲染管线的核心 trait。每个 UI 组件最终都是一个 Element。它定义了三个生命周期方法：

```rust
pub trait Element: 'static + IntoElement {
    type RequestLayoutState: 'static;
    type PrepaintState: 'static;

    // 唯一标识（用于跨帧追踪）
    fn id(&self) -> Option<ElementId>;

    // 第一阶段：请求布局
    // 返回一个 LayoutId，用于 Taffy 布局计算
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState);

    // 第二阶段：预绘制（确定边界、创建 hitbox）
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState;

    // 第三阶段：实际绘制到屏幕
    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    );
}
```

**Element 渲染管线：**

```
render() → Element Tree
    ↓
request_layout() → Taffy 计算布局 → Bounds
    ↓
prepaint() → 创建 hitbox、计算滚动偏移
    ↓
paint() → 生成 Scene 原语（Quad、Shadow、Sprite 等）
    ↓
Scene → Atlas → GPU（Metal/wgpu）
```

**一般情况下你不需要直接实现 `Element` trait。** 使用 `div()`、`text()`、`img()` 等内置元素即可。只有在需要完全自定义布局和绘制逻辑时（如实现代码编辑器），才需要手动实现 Element。

### 3.3 Render trait

`Render` 是你需要实现的核心 trait，用于声明式 UI。每个"视图"都是一个实现了 `Render` 的实体：

```rust
pub trait Render: 'static + Sized {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement;
}
```

**示例：**

```rust
struct MyView {
    count: i32,
}

impl Render for MyView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(format!("Count: {}", self.count))
            .child(
                div()
                    .bg(gpui::blue())
                    .text_color(gpui::white())
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .child("Click me")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.count += 1;
                        cx.notify(); // 触发重绘
                    })),
            )
    }
}
```

**关键要点：**
- `render()` 在每帧开始时被调用，返回一棵 Element 树
- 返回的 Element 树随后被布局、绘制，然后丢弃
- 通过 `cx.notify()` 通知框架状态变化，触发下一帧重绘

### 3.4 View 和 Model（Entity）

在 GPUI 中，`Entity<T>` 是状态的智能指针。如果 `T` 实现了 `Render`，则该 Entity 被称为"View"。

```rust
// 创建一个 Entity（同时也是一个 View，因为实现了 Render）
let entity: Entity<MyView> = cx.new(|cx| MyView { count: 0 });

// 读取实体状态
entity.read(cx, |state, _| {
    println!("Count: {}", state.count);
});

// 更新实体状态
entity.update(cx, |state, cx| {
    state.count += 1;
    cx.notify(); // 通知观察者
});
```

**Entity vs View 的区别：**
- `Entity<T>` — 任何类型的实体，用于纯状态管理
- 实现了 `Render` 的 Entity — 既是 Entity 也是 View，可以被渲染到窗口

### 3.5 RenderOnce trait

`RenderOnce` 用于创建无状态的可复用组件（类似 React 的函数组件）：

```rust
#[derive(IntoElement)]
struct Badge {
    label: SharedString,
    color: Hsla,
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_full()
            .bg(self.color)
            .text_color(gpui::white())
            .text_sm()
            .child(self.label)
    }
}

// 使用
div().child(Badge {
    label: "New".into(),
    color: gpui::red(),
})
```

### 3.6 Window 管理

窗口通过 `App::open_window()` 创建：

```rust
cx.open_window(
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(800.), px(600.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("My App".into()),
            appears_transparent: false,
            ..Default::default()
        }),
        ..Default::default()
    },
    |window, cx| {
        // 这里创建根视图
        cx.new(|cx| MyView::new(window, cx))
    },
)
.unwrap();
```

**WindowOptions 关键字段：**

```rust
pub struct WindowOptions {
    pub window_bounds: Option<WindowBounds>,    // 窗口位置和大小
    pub titlebar: Option<TitlebarOptions>,       // 标题栏配置
    pub focus: bool,                             // 是否自动聚焦
    pub show: bool,                              // 是否立即显示
    pub kind: WindowKind,                        // 窗口类型（Normal/Panel）
    pub is_movable: bool,                        // 是否可移动
    pub display_id: Option<DisplayId>,           // 目标显示器
    pub window_background: WindowBackgroundAppearance, // 背景外观
    pub window_decorations: Option<WindowDecorations>, // 装饰模式
    // ...
}
```

### 3.7 实体关系图

```
Application
  ├── App (全局上下文)
  │     ├── Entity Store (所有 Entity<T> 的数据)
  │     ├── Global Store (Global trait 存储)
  │     ├── Key Map (键绑定)
  │     ├── Background Executor
  │     └── Foreground Executor
  │
  ├── Window 1
  │     ├── Root View (Entity<dyn Render>)
  │     ├── Element Tree (每帧重建)
  │     ├── Scene (绘制原语)
  │     ├── Focus Handle
  │     └── Hitboxes
  │
  └── Window 2
        └── ...
```

---

## 四、Hello World

### 4.1 最小可运行示例

```rust
use gpui::{
    App, Bounds, Context, SharedString, Window, WindowBounds, WindowOptions,
    div, prelude::*, px, rgb, size,
};
use gpui_platform::application;

// 1. 定义视图状态
struct HelloWorld {
    text: SharedString,
}

// 2. 实现 Render trait
impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x505050))
            .size(px(500.0))
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .border_color(rgb(0x0000ff))
            .text_xl()
            .text_color(rgb(0xffffff))
            .child(format!("Hello, {}!", &self.text))
    }
}

// 3. 启动应用
fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| HelloWorld {
                    text: "World".into(),
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
```

### 4.2 逐行解释

```rust
use gpui_platform::application;
```
`gpui_platform::application()` 是跨平台入口，自动选择正确的平台后端（macOS → Metal、Windows → Win32、Linux → Wayland/X11）。

```rust
application().run(|cx: &mut App| { ... })
```
`Application::run()` 启动平台事件循环，`cx` 是 `&mut App` 类型的全局上下文。

```rust
let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
```
计算一个 500×500 像素的居中窗口边界。第一个参数 `None` 表示相对于主显示器。

```rust
cx.open_window(window_options, |_, cx| { cx.new(|_| HelloWorld { ... }) })
```
打开窗口并返回一个 `WindowHandle`。闭包中的 `cx` 是 `&mut Context<T>` 类型。`cx.new()` 创建一个新的 Entity。

```rust
cx.activate(true);
```
将应用带到前台。

### 4.3 运行

```bash
cargo run
```

---

## 五、布局系统

GPUI 的布局系统基于 [Taffy](https://github.com/DioxusLabs/taffy)，一个纯 Rust 实现的 Flexbox 和 Grid 布局引擎。

### 5.1 Flexbox 布局

```rust
impl Render for FlexExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            // 容器设置
            .flex()                    // display: flex
            .flex_row()               // flex-direction: row（默认）
            .flex_col()               // flex-direction: column
            .flex_wrap()              // flex-wrap: wrap
            .flex_nowrap()            // flex-wrap: nowrap
            .gap_4()                  // gap: 1rem
            .gap_x(px(10.0))         // column-gap: 10px
            .gap_y(px(5.0))          // row-gap: 5px
            
            // 对齐
            .justify_start()          // justify-content: flex-start
            .justify_center()         // justify-content: center
            .justify_end()            // justify-content: flex-end
            .justify_between()        // justify-content: space-between
            .justify_around()         // justify-content: space-around
            .items_start()            // align-items: flex-start
            .items_center()           // align-items: center
            .items_end()              // align-items: flex-end
            .items_stretch()          // align-items: stretch
            
            // 子项
            .flex_1()                 // flex: 1
            .flex_grow()              // flex-grow: 1
            .flex_shrink()            // flex-shrink: 1
            .flex_none()              // flex: none
            
            .child("Child 1")
            .child("Child 2")
    }
}
```

### 5.2 Grid 布局

```rust
// 来自官方 grid_layout 示例
impl Render for HolyGrailExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .grid()                   // display: grid
            .grid_cols(5)             // grid-template-columns: repeat(5, 1fr)
            .grid_rows(5)             // grid-template-rows: repeat(5, 1fr)
            .gap_1()
            .child(
                div()
                    .col_span_full()   // grid-column: 1 / -1（跨满所有列）
                    .child("Header"),
            )
            .child(
                div()
                    .col_span(1)       // grid-column: span 1
                    .h_56()
                    .child("Sidebar"),
            )
            .child(
                div()
                    .col_span(3)       // grid-column: span 3
                    .row_span(3)       // grid-row: span 3
                    .child("Content"),
            )
            .child(
                div()
                    .col_span(1)
                    .row_span(3)
                    .child("Ad"),
            )
            .child(
                div()
                    .col_span_full()
                    .child("Footer"),
            )
    }
}
```

### 5.3 尺寸和约束

```rust
div()
    // 固定尺寸
    .w(px(200.0))             // width: 200px
    .h(px(100.0))             // height: 100px
    .size(px(50.0))           // width + height: 50px
    
    // 相对尺寸
    .w_full()                 // width: 100%
    .h_full()                 // height: 100%
    .size_full()              // width + height: 100%
    .w(relative(0.5))        // width: 50%
    
    // 最大/最小约束
    .min_w(px(100.0))        // min-width: 100px
    .max_w(px(500.0))        // max-width: 500px
    .max_w_full()             // max-width: 100%
    .min_h(px(50.0))         // min-height: 50px
    .max_h(px(300.0))        // max-height: 300px
```

### 5.4 间距、内边距、边框

```rust
div()
    // 内边距
    .p(px(16.0))             // padding: 16px（四边）
    .px(px(12.0))            // padding-left + padding-right: 12px
    .py(px(8.0))             // padding-top + padding-bottom: 8px
    .pt(px(4.0))             // padding-top: 4px
    .pr(px(4.0))             // padding-right: 4px
    .pb(px(4.0))             // padding-bottom: 4px
    .pl(px(4.0))             // padding-left: 4px
    
    // Tailwind 风格的间距（基于 rem）
    .p_4()                   // padding: 1rem
    .px_2()                  // padding-left + padding-right: 0.5rem
    .py_1()                  // padding-top + padding-bottom: 0.25rem
    
    // 边框
    .border_1()              // border-width: 1px
    .border_2()              // border-width: 2px
    .border_color(gpui::blue())
    .border_t(px(2.0))      // border-top-width: 2px
    
    // 圆角
    .rounded_sm()            // small rounded corners
    .rounded_md()            // medium rounded corners
    .rounded_lg()            // large rounded corners
    .rounded_full()          // 完全圆形（pill 形状）
    .rounded(px(12.0))      // 自定义圆角半径
```

### 5.5 滚动

```rust
div()
    .id("scroll-container")   // 滚动容器必须有 id
    .overflow_y_scroll()       // overflow-y: scroll
    .overflow_x_hidden()       // overflow-x: hidden
    .size_full()
    .child("Long content...")
    .child("More content...")
```

### 5.6 定位

```rust
div()
    .relative()              // position: relative
    .child(
        div()
            .absolute()       // position: absolute
            .top(px(10.0))    // top: 10px
            .right(px(10.0))  // right: 10px
            .z_index(10)      // z-index: 10
            .child("Overlay"),
    )
```

---

## 六、内置元素详解

### 6.1 div（容器）

`div` 是 GPUI 中的万能容器，类似于 HTML 的 `<div>`。它是构建 UI 的主要构建块。

```rust
div()
    .id("unique-id")          // 唯一标识，用于跨帧追踪
    .key_context("editor")    // 键盘上下文
    .flex()
    .flex_col()
    .gap_2()
    .bg(rgb(0xffffff))
    .text_color(rgb(0x000000))
    .child("Hello")
    .child(
        div()
            .bg(gpui::red())
            .size(px(50.0))
            .rounded_full(),
    )
    .children(vec![
        "Item 1".into_any_element(),
        "Item 2".into_any_element(),
    ])
```

**div 的高级功能：**

```rust
// 样式分组（类似 CSS 的 class group）
div().group("sidebar", |div| {
    div.child(
        div()
            .group_hover("sidebar", |style| style.bg(gpui::red()))
    )
})

// 条件样式
div()
    .when(is_active, |d| d.bg(gpui::blue()))
    .when(!is_active, |d| d.bg(gpui::gray()))

// 状态样式
div()
    .hover(|style| style.bg(gpui::blue().opacity(0.1)))
    .active(|style| style.opacity(0.8))
    .focus(|style| style.border_color(gpui::blue()))
```

### 6.2 text（文本）

文本通过字符串直接渲染，GPUI 内置了文本排版系统：

```rust
// 基本文本
div().child("Hello, World!")

// 文本样式
div()
    .text_xs()               // font-size: 0.75rem
    .text_sm()               // font-size: 0.875rem
    .text_size(px(24.0))     // font-size: 24px
    .text_xl()               // font-size: 1.25rem
    .text_2xl()              // font-size: 1.5rem
    .font_weight(gpui::FontWeight::BOLD)
    .text_color(gpui::red())
    .line_height(relative(1.5))
    .text_center()           // text-align: center
    .text_left()             // text-align: left
    .text_right()            // text-align: right
    .text_decoration_2()     // text-decoration
    .text_decoration_wavy()  // wavy underline
    .text_decoration_color(gpui::red())
    .child("Styled text")

// text! 宏（用于格式化）
use gpui::text;
div().child(text!(format!("Count: {}", self.count)))

// 自定义字体
div()
    .font_family("Menlo")
    .child("Monospace text")
```

**自定义字体加载：**

```rust
// 在应用启动时加载字体文件
let fonts = vec![include_bytes!("path/to/font.ttf") as &[u8]];
cx.text_system().add_fonts(fonts).unwrap();
```

### 6.3 img（图片）

```rust
use gpui::img;

// 从 URL 加载
div().child(
    img("https://example.com/image.png")
        .size(px(256.0))              // 固定尺寸
)

// 从本地文件加载
let path: Arc<std::path::Path> = "/path/to/image.png".into();
div().child(img(path).size(px(100.0)))

// 从 AssetSource 加载
div().child(img("assets/logo.png").size(px(64.0)))

// 图片样式
img("https://example.com/photo.jpg")
    .w(px(300.0))                     // 固定宽度，高度自动
    .h(px(200.0))                     // 固定高度，宽度自动
    .max_w_full()                      // 最大宽度: 100%
    .object_fit(gpui::ObjectFit::Cover) // 填充方式
    .rounded_lg()                      // 圆角
```

**ObjectFit 选项：**

```rust
pub enum ObjectFit {
    Fill,       // 拉伸填满（可能变形）
    Contain,    // 等比缩放，完整显示
    Cover,      // 等比缩放，覆盖区域（可能裁剪）
    ScaleDown,  // 等比缩小（如果大于容器）
    None,       // 保持原始尺寸
}
```

**自定义 AssetSource：**

```rust
use gpui::AssetSource;

struct Assets {
    base: PathBuf,
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        fs::read(self.base.join(path))
            .map(|data| Some(Cow::Owned(data)))
            .map_err(|e| e.into())
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .map(SharedString::from)
                    .collect()
            })
            .map_err(|e| e.into())
    }
}

// 使用
application()
    .with_assets(Assets { base: PathBuf::from("assets") })
    .run(|cx| { ... });
```

### 6.4 svg（矢量图）

```rust
use gpui::svg;

// 从 AssetSource 加载
div().child(
    svg()
        .path("svg/icon.svg")
        .size(px(32.0))
        .text_color(gpui::red()),  // SVG 颜色通过 text_color 设置
)

// 链式样式
svg()
    .path("svg/dragon.svg")
    .size_8()                    // Tailwind 尺寸（2rem）
    .text_color(rgb(0xff0000))   // 红色
```

### 6.5 canvas（自定义绘制）

`canvas` 元素提供低级绘制 API，用于自定义图形：

```rust
use gpui::canvas;

div().child(
    canvas(
        // prepaint 回调：创建 hitbox 等
        |bounds, window, _cx| {
            // 返回的状态会传递给 paint
            let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
            hitbox
        },
        // paint 回调：实际绘制
        |bounds, hitbox, window, _cx| {
            // 绘制矩形
            window.paint_quad(gpui::fill(
                Bounds::new(
                    point(px(10.0), px(10.0)),
                    size(px(100.0), px(50.0)),
                ),
                gpui::red(),
            ));
            
            // 绘制文本
            let line = window.text_system().shape_line(
                "Custom text".into(),
                px(16.0),
                &[TextRun {
                    len: 11,
                    font: Font::default(),
                    color: gpui::black(),
                    ..Default::default()
                }],
                None,
            );
            line.paint(
                point(px(10.0), px(80.0)),
                px(20.0),
                TextAlign::Left,
                None,
                window,
                cx,
            ).unwrap();
        },
    )
    .size(px(200.0))
    .bg(gpui::white()),
)
```

**Window 上的绘制方法：**

```rust
// 绘制四边形
window.paint_quad(Quad { ... });

// 设置光标样式
window.set_cursor_style(CursorStyle::Pointer, &hitbox);

// 获取鼠标位置
let mouse_pos = window.mouse_position();

// 插入 hitbox
let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
```

### 6.6 list 和 uniform_list（列表）

**uniform_list** — 等高列表，性能最好（虚拟化）：

```rust
use gpui::uniform_list;

impl Render for MyList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            uniform_list(
                "entries",              // 唯一 id
                50,                      // 总条目数
                cx.processor(|_this, range, _window, _cx| {
                    // range 是当前可见范围
                    let mut items = Vec::new();
                    for ix in range {
                        items.push(
                            div()
                                .id(ix)
                                .px_2()
                                .cursor_pointer()
                                .on_click(move |_, _, _| {
                                    println!("clicked item {ix}");
                                })
                                .child(format!("Item {}", ix + 1)),
                        );
                    }
                    items
                }),
            )
            .h_full(),
        )
    }
}
```

**list** — 可变高度列表：

```rust
use gpui::{list, ListState, ListAlignment};

struct MyVariableList {
    list_state: ListState,
}

impl MyVariableList {
    fn new() -> Self {
        Self {
            list_state: ListState::new(
                40,                       // 总条目数
                ListAlignment::Top,       // 或 Bottom
                px(500.),                 // 初始滚动偏移
            ),
        }
    }
}

impl Render for MyVariableList {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            list(self.list_state.clone(), |index, _window, _cx| {
                let height = px(30. + (index % 5) as f32 * 10.);
                div()
                    .h(height)
                    .w_full()
                    .px_3()
                    .child(format!("Item {index}"))
                    .into_any()
            })
            .flex_1(),
        )
    }
}
```

---

## 七、事件处理

### 7.1 鼠标事件

```rust
div()
    // 点击事件
    .on_click(|event, window, cx| {
        println!("Clicked at {:?}", event.position);
        println!("Is right click: {}", event.is_right_click());
    })
    
    // 鼠标按下/释放
    .on_mouse_down(MouseButton::Left, |event, window, cx| {
        println!("Mouse down at {:?}", event.position);
    })
    .on_mouse_up(MouseButton::Left, |event, window, cx| {
        println!("Mouse up at {:?}", event.position);
    })
    .on_mouse_up_out(MouseButton::Left, |event, window, cx| {
        // 鼠标在元素外部释放
    })
    
    // 鼠标移动
    .on_mouse_move(|event, window, cx| {
        println!("Mouse at {:?}", event.position);
        println!("Pressed buttons: {:?}", event.pressed_button);
    })
    
    // 滚轮
    .on_scroll_wheel(|event, window, cx| {
        println!("Scroll delta: {:?}", event.delta);
    })
    
    // 鼠标压力（Force Touch / 数位板）
    .on_mouse_pressure(cx.listener(|this, event, _window, cx| {
        this.pressure = event.pressure;
        cx.notify();
    }))
    
    // 鼠标进入/离开
    .on_hover(|event, window, cx| {
        // 鼠标进入或离开
    })
```

### 7.2 键盘事件

键盘事件通过**动作（Action）系统**处理，这是 GPUI 的推荐方式：

```rust
// 1. 定义动作
actions!(my_app, [Save, Quit, MoveUp, MoveDown]);

// 2. 注册键绑定
cx.bind_keys([
    KeyBinding::new("cmd-s", Save, None),
    KeyBinding::new("cmd-q", Quit, None),
    KeyBinding::new("up", MoveUp, None),
    KeyBinding::new("down", MoveDown, None),
    KeyBinding::new("shift-up", MoveUp, Some("menu")), // 限定上下文
]);

// 3. 在元素上处理动作
div()
    .key_context("editor")    // 设置键盘上下文
    .on_action(|_: &Save, window, cx| {
        // 保存操作
    })
    .on_action(cx.listener(|this, _: &MoveUp, _, cx| {
        // 使用 listener 模式可以访问 self
        this.selected_index = this.selected_index.saturating_sub(1);
        cx.notify();
    }))

// 4. 在 App 级别处理动作
cx.on_action(|_: &Quit, cx| cx.quit());
```

**原始键盘事件（不推荐，但可用）：**

```rust
cx.observe_keystrokes(move |event, window, cx| {
    println!("Keystroke: {:?}", event.keystroke);
    // keystroke 包含: key, modifiers, key_char
})
.detach();
```

### 7.3 拖放

```rust
div()
    .on_drag(|event, window, cx| {
        // 开始拖拽，返回拖拽数据
        DragItem::new("my-data")
    })
    .on_drop(|data: DragItem, window, cx| {
        // 处理放下
        println!("Dropped: {:?}", data);
    })
```

### 7.4 焦点管理

```rust
struct MyWidget {
    focus_handle: FocusHandle,
}

impl Focusable for MyWidget {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MyWidget {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            // 追踪焦点
            .track_focus(&self.focus_handle)
            // 可聚焦
            .focusable()
            // Tab 键可达
            .tab_stop(true)
            // Tab 顺序
            .tab_index(1)
            // 焦点样式
            .focus(|style| style.border_color(gpui::blue()))
            // 获得焦点时执行
            .on_focus(cx.listener(|this, _, window, cx| {
                println!("Got focus!");
            }))
            // 失去焦点时执行
            .on_blur(cx.listener(|this, _, window, cx| {
                println!("Lost focus!");
            }))
            .child("Focus me")
    }
}

// 程序化控制焦点
cx.listener(|this, _, window, cx| {
    // 聚焦到某个元素
    window.focus(&this.focus_handle, cx);
    
    // 聚焦下一个/上一个
    window.focus_next(cx);
    window.focus_prev(cx);
})
```

### 7.5 Tab 焦点导航

```rust
// 来自官方 tab_stop 示例
actions!(example, [Tab, TabPrev]);

div()
    .id("app")
    .on_action(cx.listener(Self::on_tab))
    .on_action(cx.listener(Self::on_tab_prev))
    .children(self.items.iter().enumerate().map(|(ix, handle)| {
        div()
            .id(("item", ix))
            .track_focus(handle)
            .tab_index(ix as i32)
            .tab_stop(true)
            .when(handle.is_focused(window), |d| {
                d.border_3().border_color(gpui::blue())
            })
            .child(format!("Item {}", ix))
    }))

fn on_tab(&mut self, _: &Tab, window: &mut Window, cx: &mut Context<Self>) {
    window.focus_next(cx);
}

fn on_tab_prev(&mut self, _: &TabPrev, window: &mut Window, cx: &mut Context<Self>) {
    window.focus_prev(cx);
}
```

---

## 八、状态管理

### 8.1 Entity 模式

GPUI 的状态管理围绕 `Entity<T>` 展开：

```rust
// 创建实体
let counter: Entity<Counter> = cx.new(|cx| Counter {
    count: 0,
    focus_handle: cx.focus_handle(),
});

// 读取状态
let count = counter.read(cx, |c, _| c.count);

// 更新状态
counter.update(cx, |counter, cx| {
    counter.count += 1;
    cx.notify(); // 通知观察者重绘
});
```

### 8.2 共享状态

实体可以在多个视图间共享：

```rust
// 共享状态实体
let shared_state: Entity<SharedState> = cx.new(|cx| SharedState { ... });

// 在不同窗口中引用同一个实体
cx.open_window(options, |_, cx| {
    cx.new(|_| ViewA { state: shared_state.clone() })
});

cx.open_window(options, |_, cx| {
    cx.new(|_| ViewB { state: shared_state.clone() })
});

// ViewA 和 ViewB 都可以通过 shared_state 读写同一份数据
```

### 8.3 观察者模式

```rust
impl Render for ViewA {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 观察另一个实体
        cx.observe(&self.other_entity, |this, entity, cx| {
            // 当 other_entity 调用 notify() 时触发
            cx.notify(); // 重新渲染自己
        }).detach();
        
        div().child(format!("Value: {}", self.other_entity.read(cx).value))
    }
}
```

### 8.4 事件订阅

```rust
// 定义事件
struct CountChanged(i32);
impl EventEmitter<CountChanged> for Counter {}

// 发射事件
counter.update(cx, |counter, cx| {
    counter.count += 1;
    cx.emit(CountChanged(counter.count));
});

// 订阅事件
cx.subscribe(&counter, |this, entity, event: &CountChanged, cx| {
    println!("Count changed to: {}", event.0);
    cx.notify();
}).detach();
```

### 8.5 Global 状态

```rust
// 定义全局状态
struct Theme {
    primary_color: Hsla,
    font_size: f32,
}
impl Global for Theme {}

// 设置全局
cx.set_global(Theme {
    primary_color: gpui::blue(),
    font_size: 16.0,
});

// 读取全局
let theme = cx.global::<Theme>();
println!("Primary color: {:?}", theme.primary_color);

// 修改全局
cx.update_global::<Theme, _>(|theme, _| {
    theme.font_size = 20.0;
});

// 观察全局变化
cx.observe_global::<Theme>(|this, cx| {
    // Theme 变化时触发
    cx.notify();
}).detach();
```

### 8.6 异步状态更新

```rust
// 在异步任务中更新状态
cx.spawn(|this, cx| async move {
    let data = fetch_data().await;
    
    // 从异步上下文更新实体
    this.update(&cx, |this, cx| {
        this.data = data;
        cx.notify();
    }).ok();
}).detach();

// 或使用 spawn_in
cx.spawn_in(window, |this, cx| async move {
    // ...
}).detach();
```

---

## 九、样式和主题

### 9.1 内联样式（Tailwind 风格）

GPUI 提供了丰富的 Tailwind 风格链式 API：

```rust
div()
    // 颜色
    .bg(rgb(0xff0000))                   // 背景色
    .bg(gpui::blue())                    // 使用预定义颜色
    .bg(gpui::blue().opacity(0.5))       // 带透明度
    .text_color(gpui::white())           // 文字颜色
    .border_color(gpui::black())         // 边框颜色
    
    // 渐变
    .bg(linear_gradient(
        45.,                              // 角度
        linear_color_stop(gpui::red(), 0.),
        linear_color_stop(gpui::blue(), 1.),
    ))
    
    // 阴影
    .shadow_sm()
    .shadow_md()
    .shadow_lg()
    .shadow_xl()
    .shadow(vec![
        BoxShadow::new(px(0.), px(8.), hsla(0.0, 0.0, 0.0, 0.3))
            .blur_radius(px(8.))
            .spread_radius(px(2.))
            .inset(),                    // 内阴影
    ])
    
    // 透明度
    .opacity(0.5)
    
    // 光标
    .cursor_pointer()
    .cursor(CursorStyle::IBeam)
    .cursor(CursorStyle::Crosshair)
    .cursor(CursorStyle::ResizeUpDown)
```

### 9.2 颜色系统

GPUI 使用 HSLA 颜色空间：

```rust
// 预定义颜色
gpui::red()      // 红色
gpui::green()    // 绿色
gpui::blue()     // 蓝色
gpui::yellow()   // 黄色
gpui::black()    // 黑色
gpui::white()    // 白色

// 从 hex RGB
rgb(0xff0000)     // 红色
rgb(0x505050)     // 灰色

// 从 hex RGBA
rgba(0xff000080)  // 半透明红色

// HSLA 构造
hsla(0.0, 1.0, 0.5, 1.0)  // 红色 (h, s, l, a)
hsla(0.0, 0.0, 0.0, 0.5)  // 半透明灰色

// 透明颜色
transparent_black()
transparent_white()

// 颜色操作
gpui::blue().opacity(0.5)     // 设置透明度
gpui::blue().blend(gpui::red()) // 混合颜色
opaque_grey(0.5, 0.5)          // 不透明灰色
```

### 9.3 动画系统

GPUI 内置了基于时间的动画系统：

```rust
use gpui::Animation;

// 使用 Animation + AnimationExt
div()
    .with_animation(
        "my-animation",   // 唯一 id
        Animation::new(Duration::from_millis(500))
            .repeat()                    // 循环播放
            .with_easing(easing::cubic_bezier(0.4, 0.0, 0.2, 1.0)),
        |element, delta| {
            // delta 是 0.0 到 1.0 的进度值
            element.opacity(delta)
        },
    )

// 使用 request_animation_frame（帧动画）
impl Render for AnimatedView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.animating {
            self.opacity += 0.005;
            if self.opacity >= 1.0 {
                self.animating = false;
            } else {
                window.request_animation_frame(); // 请求下一帧
            }
        }
        
        div()
            .opacity(self.opacity)
            .child("Animated content")
    }
}
```

**内置缓动函数：**

```rust
use gpui::easing;

linear             // 线性
ease_in            // 缓入
ease_out           // 缓出
ease_in_out        // 缓入缓出
cubic_bezier(...)  // 自定义贝塞尔曲线
```

---

## 十、高级主题

### 10.1 多窗口

```rust
// 创建新窗口
cx.open_window(
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(400.0), px(300.0)),
            cx,
        ))),
        ..Default::default()
    },
    |_, cx| cx.new(|_| SecondWindow {}),
).unwrap();

// 在窗口间移动实体
cx.open_window(
    WindowOptions { ... },
    |_, _| entity.clone(),  // 直接返回现有实体
).unwrap();

// 关闭窗口
window.remove_window();

// 监听窗口关闭
cx.on_window_closed(|cx, window_id| {
    if cx.windows().is_empty() {
        cx.quit(); // 所有窗口关闭时退出
    }
}).detach();
```

### 10.2 异步任务

GPUI 内置了与平台事件循环集成的异步执行器：

```rust
// 后台任务
let task: Task<String> = cx.background_executor().spawn(async {
    // 在后台线程运行
    fetch_data_from_network().await
});

// 前台任务
let task = cx.foreground_executor().spawn(async {
    // 在主线程运行
});

// 在实体上下文中 spawn
cx.spawn(|this, cx| async move {
    let data = fetch_data().await;
    this.update(&cx, |this, cx| {
        this.data = data;
        cx.notify();
    }).ok();
}).detach();

// 带窗口的 spawn
cx.spawn_in(window, |this, cx| async move {
    let data = cx.background_executor().spawn(async {
        expensive_computation()
    }).await;
    
    this.update_in(cx, |this, window, cx| {
        this.result = data;
        cx.notify();
    }).ok();
}).detach();

// 定时器
cx.background_executor().timer(Duration::from_secs(1)).await;

// 延迟执行
cx.defer(|cx| {
    // 在当前帧结束后执行
});
```

### 10.3 文件对话框

GPUI 本身不直接提供文件对话框 API，但可以通过 `rfd`（Rust File Dialog）crate 集成：

```toml
[dependencies]
rfd = "0.15"
```

```rust
use rfd::FileDialog;

// 打开文件
let file = FileDialog::new()
    .add_filter("Images", &["png", "jpg", "jpeg", "gif"])
    .set_title("Select an image")
    .pick_file();

if let Some(path) = file {
    // 在后台加载图片
    cx.spawn(|this, cx| async move {
        let image = load_image(path).await;
        this.update(&cx, |this, cx| {
            this.image = Some(image);
            cx.notify();
        }).ok();
    }).detach();
}

// 保存文件
let path = FileDialog::new()
    .add_filter("Text", &["txt"])
    .set_title("Save file")
    .save_file();
```

### 10.4 系统托盘

GPUI 核心不直接提供系统托盘 API。Zed 使用的是平台特定实现。对于自己的应用，可以使用 `tray-icon` crate：

```toml
[dependencies]
tray-icon = "0.19"
```

> **注意**：这是一个独立于 GPUI 的 crate，需要在 GPUI 事件循环之外管理。

### 10.5 全局快捷键

GPUI 不直接提供全局快捷键（系统级热键）。Zed 的全局快捷键是通过操作系统特定 API 实现的。

可以使用 `global-hotkey` crate：

```toml
[dependencies]
global-hotkey = "0.6"
```

### 10.6 无障碍（AccessKit）

GPUI 通过 AccessKit 提供完整的无障碍支持：

```rust
// 来自官方 a11y 示例
div()
    .id("counter")
    .focusable()
    .tab_stop(true)
    .role(Role::SpinButton)                    // 无障碍角色
    .aria_label(format!("Counter: {}", count)) // 标签
    .aria_numeric_value(count as f64)           // 数值
    .aria_min_numeric_value(0.0)                // 最小值
    .on_a11y_action(AccessibleAction::Increment, {
        let this = cx.entity().downgrade();
        move |_, _, cx| {
            this.update(cx, |this, cx| {
                this.count += 1;
                cx.notify();
            }).ok();
        }
    })
    .on_a11y_action(AccessibleAction::Decrement, {
        let this = cx.entity().downgrade();
        move |_, _, cx| {
            this.update(cx, |this, cx| {
                this.count = (this.count - 1).max(0);
                cx.notify();
            }).ok();
        }
    })
    .on_click(cx.listener(|this, _, _, cx| {
        this.count += 1;
        cx.notify();
    }))

// ARIA 属性
div()
    .role(Role::Button)
    .role(Role::Heading)
    .role(Role::Switch)
    .role(Role::List)
    .role(Role::ListItem)
    .aria_label("My element")
    .aria_toggled(Toggled::True)
    .aria_level(1)
    .aria_position_in_set(1)
    .aria_size_of_set(3)
```

---

## 十一、实战：图片查看器

下面是一个完整的 GPUI 图片查看器示例，展示了前面介绍的大部分概念：

```rust
use gpui::{
    actions, div, img, App, Bounds, Context, Entity, EventEmitter, FocusHandle,
    KeyBinding, SharedString, Window, WindowBounds, WindowOptions,
    prelude::*, px, rgb, size, Application,
};
use gpui_platform::application;
use std::path::PathBuf;
use std::sync::Arc;

// ── 动作定义 ──────────────────────────────────────────
actions!(viewer, [OpenFile, ZoomIn, ZoomOut, ZoomFit, Quit]);

// ── 图片查看器状态 ─────────────────────────────────────
struct ImageViewer {
    focus_handle: FocusHandle,
    current_image: Option<ImageSource>,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    is_panning: bool,
}

enum ImageSource {
    Local(Arc<PathBuf>),
    Remote(String),
}

impl ImageViewer {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        
        Self {
            focus_handle,
            current_image: None,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            is_panning: false,
        }
    }

    fn open_file(&mut self, _: &OpenFile, _window: &mut Window, cx: &mut Context<Self>) {
        // 使用 rfd 打开文件对话框
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp", "svg"])
            .pick_file()
        {
            self.current_image = Some(ImageSource::Local(Arc::new(path)));
            self.zoom = 1.0;
            self.pan_x = 0.0;
            self.pan_y = 0.0;
            cx.notify();
        }
    }

    fn zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom = (self.zoom * 1.2).min(10.0);
        cx.notify();
    }

    fn zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom = (self.zoom / 1.2).max(0.1);
        cx.notify();
    }

    fn zoom_fit(&mut self, _: &ZoomFit, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        cx.notify();
    }
}

impl Focusable for ImageViewer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ImageViewer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("viewer")
            .track_focus(&self.focus_handle)
            .key_context("ImageViewer")
            .on_action(cx.listener(Self::open_file))
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::zoom_fit))
            .on_action(|_: &Quit, cx: &mut App| cx.quit())
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e1e))
            .child(
                // 工具栏
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .bg(rgb(0x2d2d2d))
                    .border_b_1()
                    .border_color(rgb(0x404040))
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0x404040))
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x505050)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                // 触发 OpenFile 动作
                                cx.dispatch_action(Box::new(OpenFile));
                            }))
                            .child("Open"),
                    )
                    .child(format!("Zoom: {:.0}%", self.zoom * 100.0))
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0x404040))
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x505050)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.zoom = 1.0;
                                this.pan_x = 0.0;
                                this.pan_y = 0.0;
                                cx.notify();
                            }))
                            .child("Fit"),
                    ),
            )
            .child(
                // 图片区域
                div()
                    .id("image-area")
                    .flex_1()
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(gpui::MouseButton::Left, |e, _, cx| {
                        // 开始平移
                    })
                    .child(if let Some(ref source) = self.current_image {
                        let image = match source {
                            ImageSource::Local(path) => img(path.clone()),
                            ImageSource::Remote(url) => img(url.clone()),
                        };
                        image
                            .max_w_full()
                            .max_h_full()
                            .object_fit(gpui::ObjectFit::Contain)
                            .into_any_element()
                    } else {
                        div()
                            .text_color(rgb(0x666666))
                            .text_xl()
                            .child("拖放图片到此处，或按 Cmd+O 打开文件")
                            .into_any_element()
                    }),
            )
    }
}

// ── 主函数 ─────────────────────────────────────────────
fn main() {
    application().run(|cx: &mut App| {
        // 注册键绑定
        cx.bind_keys([
            KeyBinding::new("cmd-o", OpenFile, None),
            KeyBinding::new("cmd-plus", ZoomIn, None),
            KeyBinding::new("cmd-equals", ZoomIn, None),
            KeyBinding::new("cmd-minus", ZoomOut, None),
            KeyBinding::new("cmd-0", ZoomFit, None),
            KeyBinding::new("cmd-q", Quit, None),
        ]);
        
        // 注册全局动作
        cx.on_action(|_: &Quit, cx| cx.quit());

        // 打开窗口
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1200.), px(800.)),
                    cx,
                ))),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("GPUI Image Viewer".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ImageViewer::new(window, cx)),
        )
        .unwrap();
        
        cx.activate(true);
    });
}
```

---

## 十二、插件系统设计

### 12.1 Zed 的 WASM 扩展架构

Zed 使用 **WebAssembly (WASM)** 作为扩展运行时，通过 `wasmtime` + `wasmtime-wasi` 实现：

```
┌─────────────────────────────────────────────┐
│               Extension Host                 │
│  ┌────────────────────────────────────────┐  │
│  │  wasmtime WASI Runtime                │  │
│  │  ┌──────────┐  ┌──────────┐           │  │
│  │  │Extension │  │Extension │           │  │
│  │  │   A      │  │   B      │           │  │
│  │  │ (WASM)   │  │ (WASM)   │           │  │
│  │  └──────────┘  └──────────┘           │  │
│  └────────────────────────────────────────┘  │
│                                              │
│  ┌────────────────────────────────────────┐  │
│  │  Extension API (zed_extension_api)     │  │
│  │  • 语言服务器管理                        │  │
│  │  • 主题注册                              │  │
│  │  • 命令注册                              │  │
│  │  • 工作区访问                             │  │
│  └────────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

**关键 crate：**
- `extension` — Extension trait 定义、manifest 解析
- `extension_api` — 扩展开发者使用的 API（通过 `wit-bindgen` 实现 WASM 组件接口）
- `extension_host` — 运行时宿主，管理扩展生命周期

**Zed 扩展的关键特征：**

1. **WASM 沙箱**：扩展运行在 WASM 沙箱中，无法直接访问文件系统
2. **WIT 接口**：使用 WebAssembly Interface Types 定义扩展 API
3. **异步通信**：宿主和扩展之间通过异步消息通信
4. **声明式 manifest**：扩展通过 `extension.toml` 声明能力

### 12.2 为自己的应用设计插件系统

对于非 Zed 应用，你可以参考以下几种方案：

**方案 A：动态库（Native Plugin）**

```toml
# 宿主应用
[dependencies]
libloading = "0.8"

# 插件
[lib]
crate-type = ["cdylib"]
```

```rust
// 插件接口
#[repr(C)]
pub struct PluginInfo {
    pub name: *const c_char,
    pub version: *const c_char,
}

// 宿主加载
unsafe {
    let lib = libloading::Library::new("plugin.so")?;
    let init: libloading::Symbol<unsafe extern "C" fn() -> PluginInfo> = lib.get(b"init")?;
    let info = init();
}
```

**方案 B：WASM（推荐，安全沙箱）**

```toml
[dependencies]
wasmtime = "25"
wasmtime-wasi = "25"
```

**方案 C：进程间通信（IPC）**

```rust
// 使用 stdin/stdout JSON-RPC
// 扩展是独立进程，通过 JSON 通信
```

**方案 D：Tauri 风格的命令系统**

```rust
// 定义命令
#[gpui::action]
struct RunPluginCommand {
    plugin_name: String,
    command: String,
    args: serde_json::Value,
}

// 注册命令处理
cx.on_action(move |cmd: &RunPluginCommand, cx| {
    let result = plugin_manager.run(&cmd.plugin_name, &cmd.command, &cmd.args);
    // 处理结果
});
```

---

## 十三、性能优化

### 13.1 渲染优化

**1. 使用 `uniform_list` 代替手动构建列表**

```rust
// ❌ 不好 — 所有项都渲染
div().children(items.iter().map(|item| item.render()))

// ✅ 好 — 只渲染可见项
uniform_list("list", items.len(), |range, _| {
    items[range].iter().map(|item| item.render()).collect()
})
```

**2. 使用 `AnyView::cached()` 缓存视图**

```rust
// 频繁变化的视图可以缓存未变化的部分
let child_view: AnyView = self.child.clone().into();
child_view.cached(StyleRefinement::default())
```

**3. 最小化 `cx.notify()` 调用**

```rust
// ❌ 不好 — 通知所有观察者
cx.notify();

// ✅ 好 — 只在真正变化时通知
if old_value != new_value {
    cx.notify();
}
```

**4. 使用 `ElementId` 稳定元素**

```rust
// ❌ 不好 — 每帧创建新 id（无法跨帧追踪）
div().id(ElementId::Integer(self.count))

// ✅ 好 — 使用稳定的 id
div().id("content-area")
```

### 13.2 内存管理

**1. 使用 `WeakEntity` 避免循环引用**

```rust
struct Parent {
    child: WeakEntity<Child>, // 弱引用
}

// 升级时检查
if let Some(child) = self.child.upgrade() {
    child.update(cx, |child, cx| { ... });
}
```

**2. 及时释放订阅**

```rust
// 存储 subscription 以便在 drop 时自动取消
struct MyView {
    _subscription: Subscription,
}

impl MyView {
    fn new(cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(...);
        Self { _subscription: subscription }
    }
}
```

**3. 避免在 render() 中分配**

```rust
// ❌ 不好 — 每帧都分配
fn render(&mut self, ...) -> impl IntoElement {
    let items: Vec<String> = (0..1000).map(|i| format!("Item {i}")).collect();
    div().children(items)
}

// ✅ 好 — 使用 lazy 迭代器
fn render(&mut self, ...) -> impl IntoElement {
    div().children((0..1000).map(|i| format!("Item {i}")))
}
```

### 13.3 大图处理策略

```rust
// 1. 使用 ObjectFit::ScaleDown 避免内存爆炸
img(large_image)
    .max_w_full()
    .max_h_full()
    .object_fit(ObjectFit::ScaleDown)

// 2. 先缩略图再全图
struct ImageViewer {
    thumbnail: Option<ImageSource>,
    full_image: Option<ImageSource>,
}

// 先加载缩略图
self.thumbnail = Some(load_thumbnail(path));
cx.notify();

// 后台加载全图
cx.spawn(|this, cx| async move {
    let full = load_full_image(path).await;
    this.update(&cx, |this, cx| {
        this.full_image = Some(full);
        cx.notify();
    }).ok();
}).detach();

// 3. 使用 ImageCache
// GPUI 内置了图片缓存，通过 img() 元素自动管理
```

---

## 十四、与 Qt/Tauri/Slint/Iced 对比

### 14.1 特性对比表

| 特性 | GPUI | Qt (Rust) | Tauri | Slint | Iced |
|------|------|-----------|-------|-------|------|
| **语言** | Rust | Rust (binding) | Rust + JS/TS | Rust + DSL | Rust |
| **渲染** | GPU (Metal/DX/Vulkan) | CPU/GPU | WebView | GPU (Skia) | GPU (wgpu) |
| **布局** | Flexbox + Grid | 自有系统 | CSS | 自有系统 | 自有系统 |
| **样式** | Tailwind API | QSS/CSS | CSS | 自有 DSL | 内联样式 |
| **性能** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| **跨平台** | 桌面 + Web | 全平台 | 全平台 | 全平台 | 全平台 |
| **学习曲线** | 中等 | 陡峭 | 平缓 | 平缓 | 中等 |
| **生态** | 早期 | 成熟 | 成长中 | 成长中 | 成长中 |
| **无障碍** | AccessKit | 完善 | 依赖 Web | 基础 | 基础 |
| **JS 集成** | ❌ | 通过 QML | ✅ 原生 | ❌ | ❌ |

### 14.2 什么时候选 GPUI

✅ **选择 GPUI：**
- 你需要极致的渲染性能（编辑器、终端、数据可视化）
- 你的团队熟悉 Rust 和 Web 开发（Flexbox + Tailwind）
- 你在构建开发者工具或 IDE
- 你需要深度自定义 UI（不只是标准控件）
- 你喜欢 Zed 的架构和设计哲学

### 14.3 什么时候不选 GPUI

❌ **不选 GPUI：**
- 需要快速搭建原型（选 Tauri）
- 需要移动端支持（选 Tauri 或 Slint）
- 需要丰富的现成组件库（选 Qt 或 Tauri + UI 框架）
- 团队主要是 Web 开发者，不想写 Rust（选 Tauri）
- 需要成熟稳定的生产环境（选 Qt）
- 需要拖拽式 UI 设计器（选 Slint）

### 14.4 独特优势

GPUI 的核心优势在于：
1. **纯 Rust** — 没有 C++ 依赖（Qt），没有 WebView（Tauri），没有 JS runtime
2. **GPU 原生** — 直接操作 GPU，不经过浏览器渲染管线
3. **Web 开发者友好** — Flexbox 布局 + Tailwind 样式 API
4. **Zed 验证** — 已在 Zed 编辑器中经过大规模实战验证
5. **WASM 扩展** — 内置的 WASM 扩展系统设计

---

## 十五、资源和社区

### 15.1 官方资源

- **GitHub 仓库**：<https://github.com/zed-industries/zed>
- **GPUI crate**：<https://crates.io/crates/gpui>
- **API 文档**：<https://docs.rs/gpui>
- **Zed 官网**：<https://zed.dev>
- **Zed Blog**：<https://zed.dev/blog>（包含 GPUI 技术文章）

### 15.2 社区

- **Zed Discord**：<https://zed.dev/community-links>
- **GitHub Discussions**：<https://github.com/zed-industries/zed/discussions>

### 15.3 学习路径

1. **第一步**：阅读本教程，运行 Hello World
2. **第二步**：研究 Zed 官方示例（`crates/gpui/examples/`）
3. **第三步**：阅读 GPUI 源码中的文档注释
4. **第四步**：研究 Zed 编辑器源码中的实际用法
5. **第五步**：构建自己的小项目

### 15.4 官方示例清单

| 示例 | 展示内容 |
|------|---------|
| `hello_world` | 最小应用、基本样式 |
| `image` | 图片加载（本地、远程、Asset） |
| `input` | 文本输入、自定义 Element |
| `text` | 字体排版、字体切换 |
| `svg` | SVG 渲染 |
| `grid_layout` | CSS Grid 布局 |
| `uniform_list` | 虚拟化列表 |
| `list_example` | 可变高度列表 |
| `shadow` | 阴影效果大全 |
| `opacity` | 透明度、动画 |
| `pattern` | 图案填充、渐变 |
| `set_menus` | 系统菜单 |
| `on_window_close_quit` | 多窗口、窗口生命周期 |
| `move_entity_between_windows` | 实体跨窗口移动 |
| `tree` | 深层嵌套性能测试 |
| `window_shadow` | 自定义窗口装饰、标题栏 |
| `tab_stop` | Tab 焦点导航 |
| `a11y` | 无障碍（AccessKit） |
| `mouse_pressure` | 鼠标压力感应 |

### 15.5 贡献指南

1. Fork [zed-industries/zed](https://github.com/zed-industries/zed)
2. 创建功能分支
3. 在 `crates/gpui/examples/` 中添加示例
4. 确保所有平台编译通过
5. 提交 PR

### 15.6 已知局限

| 局限 | 状态 |
|------|------|
| 无内置表单控件（下拉框、滑块等） | 需要自建 |
| 无内置动画状态机 | 只有基础 Animation |
| 无内置主题切换系统 | 需要自行实现 |
| 文档仍在完善中 | 持续改进中 |
| API 仍在变化 | 未到 1.0 |
| 移动端不支持 | 暂无计划 |
| 全局快捷键不内置 | 需要第三方 crate |
| 系统托盘不内置 | 需要第三方 crate |
| 文件对话框不内置 | 需要 rfd crate |

---

## 附录 A：GPUI 速查表

### 常用导入

```rust
use gpui::{
    // 核心
    App, Context, Window, Entity, SharedString,
    
    // 元素
    div, img, svg, canvas, text,
    
    // 列表
    uniform_list, list, ListState, ListAlignment,
    
    // 布局/样式
    prelude::*, px, relative, size, point,
    
    // 颜色
    rgb, rgba, hsla, Hsla,
    
    // 事件
    MouseButton, ClickEvent, KeyDownEvent,
    
    // 动作
    actions, KeyBinding, Action,
    
    // 窗口
    WindowOptions, WindowBounds, Bounds, TitlebarOptions,
    
    // 其他
    FocusHandle, Focusable, SharedUri, BoxShadow,
};
use gpui_platform::application;
```

### 模式速查

```rust
// 创建应用
application().run(|cx: &mut App| { ... });

// 创建窗口
cx.open_window(options, |_, cx| cx.new(|_| MyView { ... })).unwrap();

// 创建实体
let entity = cx.new(|cx| MyStruct { ... });

// 读取实体
entity.read(cx, |state, _| { ... });

// 更新实体
entity.update(cx, |state, cx| { ... });

// 通知重绘
cx.notify();

// 注册动作
cx.bind_keys([KeyBinding::new("cmd-s", Save, None)]);
cx.on_action(|_: &Save, cx| { ... });

// 异步任务
cx.spawn(|this, cx| async move { ... }).detach();

// 全局状态
cx.set_global(MyGlobal { ... });
cx.global::<MyGlobal>();
```

---

> **本教程最后更新**：基于 Zed commit `main` 分支，GPUI v0.2.2
>
> GPUI 仍在积极开发中，API 可能会变化。建议始终参考最新源码。
