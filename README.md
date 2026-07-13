<h1 align="center">Image Forge</h1>

<p align="center">
  <sub>一个基于 Tauri 2 + Vue 3 + Rust 的本地 AI 生图工作台 · Images API · Queue · Gallery · Templates</sub>
</p>

<p align="center">
  <img alt="version" src="https://img.shields.io/badge/version-0.2.8-9B7BEE?style=flat-square">
  <img alt="platform desktop" src="https://img.shields.io/badge/platform-desktop-111827?style=flat-square">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white">
  <img alt="Vue" src="https://img.shields.io/badge/Vue-3-42B883?style=flat-square&logo=vuedotjs&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-backend-B7410E?style=flat-square&logo=rust&logoColor=white">
  <img alt="Images API" src="https://img.shields.io/badge/API-Images-F97316?style=flat-square">
</p>

## 简介

Image Forge 是一个本地桌面生图工作台。它通过 OpenAI 兼容的 Images API 做文生图和参考图编辑，把 API 源、生成队列、历史结果、图库、提示词模板和提示词片段放在一个轻量桌面应用里。

它不依赖 Python WebUI，也不把数据丢给外部数据库。设置、队列、历史、图库和模板都写入本机应用数据目录，生成图片保存到本地输出目录。

## 功能

- OpenAI 兼容 Images API：支持 `/images/generations` 文生图和 `/images/edits` 参考图编辑；
- 多模型管理：API 源可标记为生图模型或对话模型，顶部栏分别选择“生图模型”和“对话模型”；
- 生图队列：任务入队、后台执行、取消、重试、移到队首、运行状态轮询和失败自动重试；
- 结果预览：选择队列或历史任务后预览输出图片，支持在 Finder 中定位和保存到图库；
- 图库管理：导入本地图片、保存生成结果、编辑名称/分类/备注，并作为参考图复用；
- 提示词模板：保存常用模板，支持插入、替换、收藏字段和使用次数记录；
- 提示词片段：维护短标签片段，快速插入到当前提示词；
- 参考图工作流：支持多张参考图，自动生成本地预览，并可从图库继续添加；
- 本地持久化：设置、队列、历史、图库、模板和片段写入应用数据目录；
- 马卡龙偏紫界面：三栏工作台、小型按钮、可拖拽调整 panel 宽度，默认把弹性空间留给结果预览。

## 界面结构

Image Forge 当前是单页三栏布局：

- 左侧：生图队列和历史记录；
- 中间：任务状态、结果预览、定位和入图库；
- 右侧：生成参数、提示词输入、模板/片段/图库入口和参考图条；
- 顶部：生图模型、对话模型、API 源、图库、模板、片段和设置入口。

## 数据与架构

项目没有使用传统数据库，而是用 Tauri 应用数据目录里的 JSON 文件和图片文件组成轻量本地数据层：

```text
app_data_dir/
  settings.json
  queue.json
  history.json
  prompt-snippets.json
  prompt-templates.json
  requests/
  outputs/
  gallery/
```

更多代码分层、数据流和队列运行逻辑见技术设计文档：

```text
docs/technical-design.md
```

## 开发

安装依赖：

```bash
pnpm install
```

启动完整 Tauri 桌面开发模式：

```bash
pnpm tauri dev
```

只查看 Vue 前端界面：

```bash
pnpm run dev
```

Vite 调试服务默认运行在 `http://127.0.0.1:1421`。浏览器里没有 Tauri 桌面运行时，所以调用本地文件、弹窗、队列命令等桌面能力会报错；需要完整功能调试时使用 `pnpm tauri dev`。

## 检查

常用检查：

```bash
pnpm build
cd src-tauri
cargo fmt --check
cargo check
cargo test
```

## 发布

单独升级版本：

```bash
pnpm run patch -- 0.2.6
```

打正式包：

```bash
pnpm run release -- 0.2.6
pnpm run release
```

传入版本号时会先检查新版本必须高于当前版本，再同步修改项目版本、窗口 title、页面 title 和 Rust User-Agent；不传版本号时使用当前版本。发布产物会复制到 `release/` 目录，文件名带版本号。`release/` 不提交进 Git。

## 目录

- `src/App.vue`：单页控制器，集中管理状态、轮询和 Tauri 命令调用；
- `src/components/`：主界面面板、抽屉、弹窗和任务卡片；
- `src/lib/`：前端默认模型、选项、格式化函数和 Naive UI 主题；
- `src/styles.css`：整体布局、马卡龙紫配色、panel 尺寸、拖拽条和响应式细节；
- `src/tauri.js`：前端调用 Tauri 命令与文件协议转换的轻封装；
- `src-tauri/src/commands.rs`：前端可调用的 Tauri 命令；
- `src-tauri/src/models.rs`：Rust 与前端通信的数据模型；
- `src-tauri/src/store.rs`：JSON 文件数据库、路径管理和数据归一化；
- `src-tauri/src/services/`：Images API 调用和后台队列 worker；
- `src-tauri/src/lib.rs`：Tauri 入口和命令注册；
- `src-tauri/tauri.conf.json`：Tauri 窗口、打包、权限和应用版本配置；
- `docs/technical-design.md`：技术架构、数据流、存储结构和运行逻辑说明。

## 调整 UI 的入口

- 改三栏默认宽度：修改 `src/App.vue` 里的 `panelSizes` 和 `workspaceStyle`；
- 改拖拽范围：修改 `src/App.vue` 的 `startPanelResize()`；
- 改顶部模型选择：修改 `src/components/AppTopbar.vue`；
- 改 API 源/模型管理：修改 `src/components/dialogs/ApiSourceDialog.vue`；
- 改工作台参数区：修改 `src/components/ComposerPanel.vue`；
- 改队列/结果预览：修改 `src/components/QueuePanel.vue` 和 `src/components/ResultPanel.vue`；
- 改配色：优先修改 `src/lib/theme.js`，再修改 `src/styles.css`；
- 改窗口最小尺寸：修改 `src-tauri/tauri.conf.json` 的 `minWidth` 和 `minHeight`。

## Git 与版本规则

- 本项目使用 PNPM 管理前端依赖；
- 代码提交遵循 Conventional Commits，例如 `feat: 增加队列工作台`；
- 每次会话提交代码时都需要升级版本，并把版本差异写入本 README；
- 每个正式版本都需要产出 `.dmg` 和 `.app` 到 `release/`，但不要提交这些二进制产物。

## 版本记录

### v0.2.6

- README 改为居中标题、徽章和产品化章节结构，参考 AnyForge 的展示风格。
- 修复版本升级脚本，使其能更新拆分后的 `src-tauri/src/defaults.rs` User-Agent。
- 新增 `pnpm run patch -- <版本号>` 脚本入口，保持 README 中的发布命令可直接执行。

### v0.2.5

- 前端拆分为组件化结构，`App.vue` 收敛为页面控制器，弹窗和抽屉全部改为 Vue Template 语法。
- Rust 后端按 `models / store / services / commands / utils / state / defaults` 分层拆分。
- API 源管理新增生图模型/对话模型类型，顶部栏分别选择生图模型和对话模型。
- 内部 provider ID 改为自动随机生成并在界面中隐藏。
- 新增 `docs/technical-design.md`，说明技术架构、数据存储、运行逻辑和文件职责。
- 图标清理流程保留 `src-tauri/icons/icon.png`，避免 Tauri 宏和编辑器找不到图标。
- `tauri.conf.json` 改用本地 Tauri schema，避免编辑器远程 schema 信任警告。

### v0.2.4

- 统一 `release` 发布脚本算法：支持可选版本参数、版本升序检查、重建图标、跨平台产物收集和过程文件清理。
- 新增 `pnpm run patch -- <版本号>`，用于单独同步项目版本、窗口 title 和页面 title。
- 生成图标改为发布过程文件，仓库只保留 `src-tauri/icons/app-icon.png` 源图。

### v0.2.3

- 新增 `pnpm run release -- <版本号>` 发布脚本，可选更新版本号、重建图标、打包、整理发布产物并清理过程文件。

### v0.2.2

- API 源管理新增“导入”按钮，可粘贴 KiloCode/OpenAI 风格 JSON 配置。
- 导入时从配置 key 推导 API 源名称和 ID，并映射 Base URL、API Key、图像模型字段。
- Provider ID 归一化改为保留英文字母大小写。

### v0.2.1

- 增加 `release/` 目录作为本地发布产物目录，并加入 Git 忽略规则。
- 规定每个版本的 `.dmg` 和 `.app` 产物文件名都必须带版本号。
- 清理 `src-tauri/target` 构建过程目录，避免把构建缓存留在工作区。
- README 增加 Vue 前端调试服务、完整 Tauri 调试方式和发布产物规则。

### v0.2.0

- 调整工作台布局：队列在左、结果预览居中、生成工作台在右，默认弹性空间给结果预览。
- 工作台内部把参数配置放到提示词框上方，并统一小型按钮和控件尺寸。
- 新增 panel 拖拽调整能力，支持在不同 panel 之间的留白处 resize。
- 约束 Tauri 主窗体最小尺寸为 `1200x900`。
- 移除 Responses API、主模型和联网搜索相关兼容逻辑，只保留 Images API。
- 删除 `example` 目录，补全 `.gitignore`，清理移动端图标等不需要上传的产物。
- README 改为中文，补充功能说明、UI 修改入口和版本规则。

### v0.1.0

- 初始化 Tauri 版 Image Forge。
- 建立 Vue 3 + Naive UI 前端、Rust 后端命令、应用图标和 GitHub 远程仓库关联。
- 实现 API 源管理、生图队列、历史、结果预览、图库、提示词模板和提示词片段的基础能力。
