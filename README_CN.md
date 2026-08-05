# Lumia

<p align="center">
  <img src="crates/lumia-app/resources/logo.png" alt="Lumia Logo" width="256">
</p>

<p align="center">
  <a href="https://github.com/iFence/Lumia/releases"><img src="https://img.shields.io/github/v/release/iFence/Lumia?style=flat-square&color=blue" alt="Release"></a>
  <a href="https://github.com/iFence/Lumia/releases"><img src="https://img.shields.io/github/downloads/iFence/Lumia/total?style=flat-square&color=green" alt="Downloads"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/iFence/Lumia?style=flat-square&color=orange" alt="License"></a>
  <img src="https://img.shields.io/badge/MSRV-1.95-red?style=flat-square" alt="MSRV">
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?style=flat-square" alt="Platform">
  <a href="https://github.com/iFence/Lumia"><img src="https://img.shields.io/github/stars/iFence/Lumia?style=social" alt="Stars"></a>
</p>

<p align="center">
  <a href="README.md">English</a>
</p>

Lumia 是一款小巧、精致、高性能的跨平台图片浏览器，使用 Rust、GPUI 和 `gpui-component` 构建。

产品的目标是打造一款启动迅速、内存占用低、运行稳定的图片浏览器，既能满足日常图片浏览需求，也能服务于摄影师、UI 设计师和工程师的专业预览工作流。核心应用负责桌面窗口、浏览状态、快速导航和插件宿主。较重的功能则隔离在进程插件中，使其可以独立演进，不会拖慢或影响核心浏览器的稳定性。

## 能力模型

Lumia 围绕四个能力层级组织：

| 层级 | 包含的能力 | 架构边界 |
|---|---|---|
| 核心浏览器 | 图片预览；缩放、平移和显示旋转；图片信息；EXIF 显示；文件夹浏览；基本排序、筛选和收藏；常见格式的快速预览 | 内置于应用中，针对启动时间、打开延迟、内存占用和稳定性进行优化 |
| 内置轻量编辑 | 旋转、裁剪、镜像、调整大小、简单压缩、简单色彩调整和导出副本 | 仅在操作轻量且面向副本导出的情况下内置 |
| 官方插件 | 捆绑的 PSD/PSB 合成预览、可选的 RAW 预览，以及未来的 HDR、HEIC/HEIF、高级格式预览和简单格式转换 | 通过进程插件协议实现；每个插件可根据体积和依赖选择随应用捆绑或单独发布 |
| 可选插件 | AI 风格化、背景移除、超分辨率、修复、外扩、降噪、批量水印、批量转换、压缩插件、云端模型插件和本地模型插件 | 通过相同的进程插件边界单独安装或启用 |

当前实现状态：单图预览、缩放、平移、显示旋转、图片信息、相邻图片导航、相邻预加载、设置、轻量裁剪/调整大小副本导出、stdio JSON-RPC 插件协议、捆绑的 PSD/PSB 合成预览、可选的签名 RAW 预览，以及声明式插件 UI 贡献均已就位。完整的文件夹浏览界面、收藏、筛选、更多专业格式插件和 AI/批量插件是产品目标，尚未完全实现。

### 超大图片

对于超过 Lumia 安全解码内存或 GPU 纹理限制的常见光栅图像，使用进程内渐进式路径。Lumia 首先生成一个有界预览，然后准备一个磁盘支持的 BGRA 缓存，仅在用户缩放或平移时加载可见的 512×512 图块。PNG 逐行处理；对于当前纯 Rust 解码器需要完整目标缓冲区的格式，使用临时内存映射文件代替数 GB 的 Rust 堆分配。

缓存存储在操作系统临时目录下，上限为 8 GiB，启动时清除不完整或超过一周的条目。非常大的 JPEG 和 WebP 文件可能暂时需要接近其解码像素大小的磁盘空间。非常大的 GIF 文件目前在此渐进式路径中显示第一帧。

## 安装

### Windows

从 [Releases](https://github.com/iFence/Lumia/releases) 页面下载推荐的安装程序（`Lumia-Setup-*-x64.exe`）或便携版压缩包（`lumia-portable-windows-x64.zip`）。

- **安装程序（推荐）**：选择简体中文或 English，然后按照安装向导操作。Lumia 及其官方 Photoshop 预览插件安装在 `%LOCALAPPDATA%\Programs\Lumia` 下，通常不需要管理员权限。始终创建开始菜单快捷方式；可选的桌面快捷方式默认不创建。安装程序在继续之前还会移除检测到的旧版 `Program Files` 安装。
- **MSI 包**：提供单独的 `en-US` 和 `zh-CN` MSI 文件，用于静默部署和故障排除。它们使用相同的每用户默认设置，但从旧版每机器 MSI 迁移必须使用安装程序或先卸载旧版本。
- **便携版**：解压完整的 `.zip` 压缩包并运行 `lumia-app.exe`。保持包含的 `plugins` 目录与应用程序放在一起。要添加右键菜单支持，运行一次 `lumia-app --register-context-menu`。

### macOS

从 [Releases](https://github.com/iFence/Lumia/releases) 页面下载 `.dmg` 文件。打开磁盘映像并将 **Lumia.app** 拖入 `Applications` 文件夹。安装后，在 Finder 中右键单击任意图片并选择 **打开方式 -> Lumia**，或在 Lumia 中使用 **设置 -> 文件关联** 选择默认格式。

如果 macOS 提示 **“Lumia.app 已损坏，无法打开”**，请打开“终端”并执行以下命令，然后重新启动 Lumia：

```bash
sudo xattr -dr com.apple.quarantine /Applications/Lumia.app
```

请仅对从官方 [Releases](https://github.com/iFence/Lumia/releases) 页面下载的 Lumia 执行此命令。

如果你更喜欢便携版二进制文件，运行 `lumia-app --register-context-menu` 在 `~/Applications/` 下创建一个包装应用包，以便 Lumia 出现在 Finder 的"打开方式"菜单中。

### Linux

从 [Releases](https://github.com/iFence/Lumia/releases) 页面下载压缩包（`lumia-linux-x64.tar.gz`）并解压：

```bash
tar -xzf lumia-linux-x64.tar.gz
cd lumia-release
```

运行包含的 `install.sh` 脚本来安装应用程序、官方插件、桌面入口和图标：

```bash
./install.sh
```

这会在系统的右键"打开方式"菜单中为所有支持的图片格式注册 Lumia。使用 **设置 -> 文件关联** 将 Lumia 设为选定格式的默认应用。要卸载，运行 `./install.sh --uninstall`。

如果你仅下载了原始应用程序二进制文件，由于缺少官方插件，PSD/PSB 预览不可用。你仍然可以手动注册核心浏览器：

```bash
lumia-app --register-context-menu      # 添加 .desktop 入口和图标
lumia-app --unregister-context-menu    # 移除它们
```

> **Linux 依赖**：需要安装 GPU 驱动、`xdg-utils` 和系统库（fontconfig、wayland、xkbcommon、xcb）。在 Debian/Ubuntu 上：
> ```bash
> sudo apt install xdg-utils shared-mime-info libfontconfig-dev libwayland-dev libxkbcommon-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-x11-dev
> ```

## 文件关联与默认应用

Lumia 与操作系统的"打开方式"菜单和默认应用设置集成。在 **设置 -> 文件关联** 中选择格式后，双击这些图片即可在 Lumia 中打开。如果 Lumia 已经在运行，现有窗口会被激活并加载新图片。

| 平台 | 默认应用行为 |
|---|---|
| **Windows** | 在当前用户下注册选定格式，并打开 Windows 默认应用设置供确认。 |
| **macOS** | 通过 Launch Services 应用选定的查看器处理程序。 |
| **Linux** | 通过 `xdg-mime` 应用选定的 MIME 处理程序。 |

Windows 10 和 11 需要用户在系统设置中确认默认应用。Lumia 注册选定格式并打开其 Windows 默认应用页面；它不会直接覆盖受保护的用户选择。

在 macOS 和 Linux 上，清除格式会恢复 Lumia 首次接管时捕获的处理程序，但仅当 Lumia 仍是当前默认应用时有效。Lumia 永远不会覆盖外部更改的默认应用。如果不知道之前的处理程序，设置页面会标识需要手动选择的格式。

## 官方捆绑插件

捆绑的 Photoshop 预览插件会随 Lumia 一起安装、升级和移除。用户无需单独下载或复制它。发布产物保持以下应用程序相对布局：

```text
Lumia/
  lumia-app[.exe]
  plugins/
    lumia-plugin-photoshop/
      lumia-plugin-photoshop[.exe]
      lumia.plugin.json
```

MSI、Windows 便携版 ZIP、macOS 应用包和 Linux 归档文件均包含此布局。官方 RAW 和标注插件则以单独签名的 `.lumiaplugin` 包形式发布。

## 可选 RAW 插件

官方 `lumia.raw` 插件为相机 RAW 文件提供进程隔离的预览能力，不会把 LibRaw 或其他重量级解码器放进 Lumia 的核心进程。它以单独签名的 `.lumiaplugin` 包发布，不包含在默认的 Lumia 安装程序或便携版压缩包中。即使未安装该插件，Lumia 仍能识别受支持的 RAW 文件，并显示安装和重启指引，而不是把它们当作未知图片。

该插件使用 LibRaw 0.22.2 解码最长边不超过 4096 像素、已校正方向的 8-bit sRGB PNG 预览。同时会把可用的相机品牌和型号、镜头、ISO、快门速度、光圈、焦距、拍摄时间和 GPS 坐标映射到 Lumia 的图片信息面板。

签名后的 Windows 插件包已经包含 LibRaw 及其全部原生解码依赖。用户只需安装这个 `.lumiaplugin` 包，不需要额外安装 LibRaw、zlib、JPEG 库或 Microsoft Visual C++ 运行库。

支持的扩展名按不区分大小写的方式匹配：

`.dng`、`.cr2`、`.cr3`、`.crw`、`.nef`、`.nrw`、`.arw`、`.sr2`、`.srf`、`.raf`、`.orf`、`.rw2`、`.rwl`、`.pef`、`.srw`、`.3fr`、`.fff`、`.mef`、`.mos`、`.mrw`、`.kdc`、`.dcr`、`.erf`、`.x3f` 和 `.iiq`。

首个版本中的 RAW 支持为只读模式。浏览、缩放、平移、显示旋转和图片信息仍然可用；编辑、标注以及任何基于预览像素的导出都会被禁用，以避免把该预览误认为全分辨率源数据。

安装 RAW 插件的方法：

1. 从 GitHub Release 下载与你的系统匹配的 `Lumia-RAW-<platform>-<architecture>.lumiaplugin` 文件。
2. 打开 **设置 -> 插件**，选择 **从文件安装**。
3. 选择该插件包，检查其身份和权限后点击 **安装**。
4. 重启 Lumia，然后打开受支持的 RAW 文件。

移除或升级插件也在同一个 **设置 -> 插件** 页面完成。安装前会验证包签名、负载签名、目标平台、插件 API 兼容性、路径、大小以及 SHA-256 摘要。

## 可选的标注插件

官方标注插件作为单独的包发布。没有它，Lumia 不会在图片上下文菜单中添加标注行，也不会创建标注面板。安装并重启 Lumia 后，右键单击图片并选择 **Annotate / 标注** 打开宿主渲染的面板，放置图标标记，撤销或重做更改，并导出 PNG、JPEG 或 WebP 副本而不修改源图片。

1. 从 GitHub Release 下载匹配你操作系统和 CPU 架构的 `.lumiaplugin` 文件。
2. 打开 **设置 -> 插件**，选择 **从文件安装**。
3. 选择下载的包，查看其身份、版本和请求的权限，然后选择 **安装**。
4. 重启 Lumia。右键单击图片并选择 **Annotate / 标注**。

在同一个 **设置 -> 插件** 页面中移除插件。移除会立即隐藏其贡献的命令；如果页面要求你完成应用更改，请重启 Lumia。

Lumia 在安装前验证包的签名、官方插件 ID、目标操作系统和架构、Lumia/插件 API 兼容性、每个负载路径、文件大小和 SHA-256 摘要。首个版本仅接受由 Lumia 签名的白名单插件；第三方包和针对不同平台的包将被拒绝而不会安装。

不再需要手动复制。为便于故障排除，用户安装的插件存储在以下固定目录中：

| 平台 | 插件目录 |
|---|---|
| Windows | `%APPDATA%\Lumia\plugins\` |
| macOS | `~/Library/Application Support/Lumia/plugins/` |
| Linux | `$XDG_DATA_HOME/lumia/plugins/`，或默认 `~/.local/share/lumia/plugins/` |

现有的手动复制的官方插件目录仍然可被发现，但新的安装应使用设置界面，以确保包完整性和事务性替换得到强制执行。

### 发布签名

官方发布作业需要受保护的 GitHub Actions 密钥 `LUMIA_PLUGIN_SIGNING_KEY_PEM`。它可以包含官方 Ed25519 PKCS#8 PEM 或其 base64 编码的 DER 形式，并且必须与嵌入 Lumia 的公钥匹配。签名脚本永远不会打印该密钥。缺失或不匹配的密钥会导致打包失败，Lumia 的生产验证器会在 GitHub Release 上传前再次检查最终的 `.lumiaplugin` 归档文件。

## 工作空间

- `crates/lumia-app`：GPUI 桌面窗口、基于 `gpui-component` 的 UI、浏览器编排和插件宿主集成。
- `crates/lumia-core`：浏览器状态和共享领域模型，无 UI 依赖。
- `crates/lumia-plugin-api`：宿主和插件共享的 JSON-RPC 类型。
- `crates/lumia-plugin-host`：进程插件启动器和基于换行符分隔的 stdio 传输。
- `plugins/lumia-plugin-sample`：用于验证协议的最小进程插件。
- `plugins/lumia-plugin-photoshop`：官方捆绑的 PSD/PSB 合成预览插件。
- `plugins/lumia-plugin-raw`：可选的官方相机 RAW 预览插件，基于 LibRaw，并包含原生桥接源码。
- `plugins/lumia-plugin-annotation`：可选的官方图标标注插件和签名包元数据。

## 架构

核心应用程序应保持为打开和浏览图片的快速路径。通用浏览器状态属于 `lumia-core`；UI 和事件处理属于 `lumia-app`；插件通信类型属于 `lumia-plugin-api`；插件进程管理属于 `lumia-plugin-host`。

图片数据通过路径和元数据跨越插件边界传递，而不是通过 base64 或 JSON 内联像素缓冲区。官方捆绑插件和第三方插件使用相同的清单、权限和 JSON-RPC 协议。这使得专业格式、AI、云集成、批处理和重量级原生依赖保持在核心进程之外。

支持 UI 的插件以有界协议数据的形式贡献命令、上下文菜单行、面板、控件和画布工具声明。插件不能注入 GPUI 元素或任意 HTML。Lumia 渲染每一个贡献，拥有指针速率的画布交互，验证返回的面板模型，并终止超时或格式错误的会话。

`lumia-core` 目前包含 HEIC/HEIF 解码支持作为过渡桥接。新的重量级或专业格式支持应设计为官方捆绑插件，除非未来的 ADR 明确将某项能力移入核心。

## UI 技术栈

- `gpui` 和 `gpui_platform` 来源于 `zed-industries/zed` 仓库。
- `gpui-component` 来自 `longbridge/gpui-component`，在 `crates/lumia-app/src/main.rs` 中通过 `gpui_component::init(cx)` 初始化。
- 直接的 `gpui` 和 `gpui_platform` 依赖有意使用与 `gpui-component` 相同的非固定 git URL 形式；实际的 Zed 版本通过 `Cargo.lock` 固定。
- Lumia 根视图包装在 `gpui_component::Root` 中，`crates/lumia-app/src/widgets.rs` 中的共享组件在适合交互模型的地方使用 `gpui-component` 原语。

## 开发

```powershell
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo run -p lumia-app
```

注意事项：

- 更新 GPUI 意味着通过 Cargo 更新 Zed 来源的依赖集，提交生成的 `Cargo.lock`，保持 `rust-toolchain.toml` 与锁定的版本一致，并在 ADR 中记录有意义的策略变更。
- 在更改 UI 基础设施时，验证 GPUI 表面和 `gpui-component` 集成在整个工作空间中仍能干净编译。

## 打包

### Windows MSI 安装程序

安装 `cargo-wix` 并下载 WiX Toolset 二进制文件（无需管理员权限）：

```powershell
# 安装 cargo-wix
cargo install cargo-wix --version 0.3.9

# 下载并解压 WiX Toolset 二进制文件到 %LOCALAPPDATA%
$url = "https://github.com/wixtoolset/wix3/releases/download/wix3141rtm/wix314-binaries.zip"
$dest = "$env:LOCALAPPDATA\wixtoolset"
Invoke-WebRequest -Uri $url -OutFile "$env:TEMP\wix314-binaries.zip"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Expand-Archive -Path "$env:TEMP\wix314-binaries.zip" -DestinationPath $dest -Force
```

构建两个本地化的 MSI 包和 Setup 引导程序：

```powershell
$env:WIX = "$env:LOCALAPPDATA\wixtoolset"
./scripts/build-windows-installers.ps1
# 输出：
# target/wix/Lumia-Setup-<version>-x64.exe
# target/wix/Lumia-<version>-x64-en-US.msi
# target/wix/Lumia-<version>-x64-zh-CN.msi
```

安装程序包括：
- 在 `%LOCALAPPDATA%\Programs\Lumia` 下的每用户安装
- 必需的开始菜单快捷方式和可选的桌面快捷方式
- 英文和简体中文安装界面
- 从 Lumia 设置管理的每用户文件关联
- 通过 Windows"应用和功能"进行干净卸载

验证 ICO 结构和生成的包：

```powershell
./scripts/verify-windows-icon.ps1
./scripts/verify-windows-packages.ps1 -PackageDirectory target/wix
```

### 发布

推送 `v*` 标签以触发 CI 发布工作流：

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions 将构建 Setup EXE、两个本地化的 MSI 包、便携版 zip、平台二进制文件，以及单独签名的标注和 RAW 插件归档，然后验证并附加到新的 [Release](https://github.com/iFence/Lumia/releases)。

## 图片格式策略

| 类别 | 扩展名 | 预期的支持路径 |
|---|---|---|
| 常见网页和桌面格式 | `.jpg` `.jpeg` `.png` `.gif` `.webp` `.bmp` `.ico` `.tga` `.tif` `.tiff` | 核心浏览器快速路径，依赖保持轻量 |
| 其他轻量格式 | `.avif` `.dds` `.ff` `.farbfeld` `.pbm` `.pam` `.ppm` `.pgm` `.qoi` `.svg` | 根据依赖和渲染成本决定核心或插件 |
| 专业和重型预览格式 | `.hdr` `.exr` `.heic` `.heif` `.psd` `.psb` 以及上文列出的相机 RAW 扩展名 | PSD/PSB 使用捆绑的 Photoshop 插件；RAW 使用可选的签名 `lumia.raw` 插件 |
| 转换和批量输出格式 | 由项目按插件定义 | 插件协议 |

当前注册的扩展名涵盖 19 个格式系列的 51 种扩展名。PSD/PSB 支持通过捆绑的进程插件预览存储的合成图像；它不暴露图层也不编辑 Photoshop 文档。RAW 支持使用上文描述的可选进程插件，并保持只读。注册并不意味着每个高级格式都应在核心应用内实现。

使用以下命令一起构建应用程序和捆绑的 Photoshop 插件：

    cargo build --release -p lumia-app -p lumia-plugin-photoshop

两个可执行文件输出到相同的目标配置目录，以便 Lumia 可以在应用程序旁边发现插件。
