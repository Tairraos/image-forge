<h1 align="center">Image Forge</h1>

<p align="center">
  <sub>一个基于 Tauri 2 + Vue 3 + Rust 的本地 AI 生图工作台 · Images API · Queue · Templates</sub>
</p>

<p align="center">
  <img alt="version" src="https://img.shields.io/badge/version-0.2.41-9B7BEE?style=flat-square">
  <img alt="platform desktop" src="https://img.shields.io/badge/platform-desktop-111827?style=flat-square">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white">
  <img alt="Vue" src="https://img.shields.io/badge/Vue-3-42B883?style=flat-square&logo=vuedotjs&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-backend-B7410E?style=flat-square&logo=rust&logoColor=white">
  <img alt="Images API" src="https://img.shields.io/badge/API-Images-F97316?style=flat-square">
</p>

## 简介

Image Forge 是一个本地桌面生图工作台。它通过 OpenAI 兼容的 Images API 做文生图和参考图编辑，把 API 源、生成历史、结果预览和提示词模板维护放在一个轻量桌面应用里。

它不依赖 Python WebUI，也不把数据丢给外部数据库。设置、队列、历史和模板都写入本机应用数据目录，生成图片保存到本地输出目录。

## 功能

- OpenAI 兼容 Images API：支持 `/images/generations` 文生图和 `/images/edits` 参考图编辑；
- 多模型管理：API 源可标记为生图模型或对话模型，支持独立代理、调用 `/models` 获取模型列表；
- 生图历史：任务入队、后台执行、刷新、重试、删除、运行状态轮询和失败自动重试，历史卡片显示真实图片尺寸；
- 结果预览：选择历史任务后预览输出图片，支持复制、下载到 Downloads 和在 Finder 中定位；
- 提示词模板：支持增删查改、参考图、工作台快捷保存、AI 填充 `{}` 占位和引用到提示词光标位置；
- 模板包导入导出：ZIP 同时包含版本化清单、可读 Markdown 和哈希图片，可直接导回并跳过重复模板；
- 参考图工作流：支持文件选择、剪贴板图片、Finder 文件复制/剪切粘贴和文件拖放，按 SHA-256 去重持久化，历史重用和模板引用会恢复参考图；
- 安全删除：历史、模板和 API 源删除前需要确认；参考图可直接从当前草稿移除；生成图及无人引用的参考图进入系统回收站；
- 本地持久化：设置、队列、历史、请求、模板和参考图都保存在本机应用数据目录；
- 马卡龙偏紫界面：三栏工作台、小型按钮、可拖拽调整 panel 宽度，默认把弹性空间留给结果预览。

## 界面结构

Image Forge 当前是单页三栏布局：

- 左侧：生成历史记录；
- 中间：任务状态、结果预览、详情和重用；
- 右侧：生成参数、生图模型、参考图、提示词输入、存为模板和引用模板入口；
- 顶部：品牌、API 源、模板维护和关于入口；运行数、排队数和提示信息显示在底部状态栏。

## 数据与架构

项目没有使用传统数据库，而是用 Tauri 应用数据目录里的 JSON 文件和图片文件组成轻量本地数据层：

```text
app_data_dir/
  settings.json
  queue.json
  history.json
  prompt-templates.json
  requests/
  outputs/
  references/
  clipboard/
```

`references/` 保存任务和模板共享的参考图资源，文件名使用内容哈希；`clipboard/` 仅作为旧版兼容目录保留。

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
pnpm run patch -- <next-version>
```

打正式包：

```bash
pnpm run release -- <next-version>
pnpm run release
```

传入版本号时会先检查新版本必须高于当前版本，再同步修改项目版本、窗口 title、页面 title 和 Rust User-Agent；不传版本号时使用当前版本。发布产物会复制到 `release/` 目录，文件名带版本号；新版 `.app` 和 `.dmg` 生成后，旧版本发布包会移入系统回收站。`release/` 不提交进 Git。

## 目录

- `src/App.vue`：单页控制器，集中管理状态、轮询和 Tauri 命令调用；
- `src/components/`：主界面面板、弹窗和任务卡片；
- `src/lib/`：前端默认模型、选项、格式化函数和 Naive UI 主题；
- `src/styles.css`：整体布局、马卡龙紫配色、panel 尺寸、拖拽条和响应式细节；
- `src/tauri.js`：前端调用 Tauri 命令、文件打开/保存对话框和原生拖放事件的轻封装；
- `src-tauri/src/commands.rs`：前端可调用的 Tauri 命令；
- `src-tauri/src/models.rs`：Rust 与前端通信的数据模型；
- `src-tauri/src/store.rs`：JSON 文件数据库、路径管理和数据归一化；
- `src-tauri/src/services/`：Images/Chat/Models API、后台队列、剪贴板、参考图资源和模板 ZIP 导入导出；
- `src-tauri/src/lib.rs`：Tauri 入口和命令注册；
- `src-tauri/tauri.conf.json`：Tauri 窗口、打包、权限和应用版本配置；
- `docs/technical-design.md`：技术架构、数据流、存储结构和运行逻辑说明。

## 调整 UI 的入口

- 改三栏默认宽度：修改 `src/App.vue` 里的 `panelSizes` 和 `workspaceStyle`；
- 改拖拽范围：修改 `src/App.vue` 的 `startPanelResize()`；
- 改顶部 API 源/模板入口：修改 `src/components/AppTopbar.vue`；
- 改 API 源/模型管理：修改 `src/components/dialogs/ApiSourceDialog.vue`；
- 改工作台参数区和生图模型选择：修改 `src/components/ComposerPanel.vue`；
- 改队列/结果预览：修改 `src/components/QueuePanel.vue` 和 `src/components/ResultPanel.vue`；
- 改模板维护：修改 `src/components/dialogs/TemplateManagerDialog.vue`；列表显示标题、参考图数量和操作，模板内容在查看/编辑弹窗中维护；
- 改模板引用和 AI 填充：修改 `src/components/dialogs/TemplateReferenceDialog.vue` 和 `src-tauri/src/services/chat.rs`；
- 改模板包导入导出：修改 `src/App.vue`、`src-tauri/src/services/template_bundle.rs` 和 `src/tauri.js`；
- 改参考图持久化：修改 `src-tauri/src/services/references.rs` 和 `src-tauri/src/services/clipboard.rs`；
- 改配色：优先修改 `src/lib/theme.js`，再修改 `src/styles.css`；
- 改窗口最小尺寸：修改 `src-tauri/tauri.conf.json` 的 `minWidth` 和 `minHeight`。

## Git 与版本规则

- 本项目使用 PNPM 管理前端依赖；
- 代码提交遵循 Conventional Commits，例如 `feat: 增加队列工作台`；
- 每次会话提交代码时都需要升级版本，并把版本差异写入本 README；
- 每个正式版本都需要产出 `.dmg` 和 `.app` 到 `release/`，但不要提交这些二进制产物。

## 版本记录

### v0.2.42

- 主工作台点击“清空”时，同时清空提示词和当前参考图。

### v0.2.41

- 支持从 macOS 原生剪贴板读取 Finder 图片文件引用，复制或剪切文件后可直接粘贴为参考图。
- Finder 剪贴板优先读取原始图片路径，普通位图剪贴板继续按内容哈希保存到参考图目录。
- 引用模板页脚的对话模型下拉菜单改为向上展开，避免选项超出窗口底部。

### v0.2.40

- 修复主工作台、模板新增/编辑和模板引用中的参考图删除按钮无响应。
- 模板新增和编辑弹窗支持把图片拖到模板内容区或“参考图”按钮，图片会加入当前模板草稿。
- 原生 Tauri 拖放按主工作台和模板编辑器分别路由，避免参考图加入错误区域。

### v0.2.39

- 将模板保存、模板导入和 AI 填充的成功/失败提示改为 Vue 通知弹窗。
- 移除所有原生 `window.alert/confirm`；单按钮通知弹窗打开后自动聚焦按钮，Enter 和 Esc 都可以关闭。

### v0.2.38

- 参考图移除改为立即执行，不再弹出确认框。
- 历史任务、模板和 API 源删除统一使用应用内确认弹窗。
- 确认弹窗打开后自动聚焦“确认”按钮，支持回车确认和 Esc 取消。

### v0.2.37

- 修复新添加参考图的删除按钮点击区域，参考图仍需确认后移除。
- 粘贴 Finder 文件时识别绝对路径和 `file://` URI；只有图像文件会加入参考图，非图像路径静默忽略。
- 修复拖放图片无响应，Tauri 原生拖放和 WebView 数据传递都会提取文件路径并通过图片内容校验。

### v0.2.36

- 模板新增可编辑标题；标题为空时取内容第一行，并限制为最多 24 个 Unicode 字符。
- 模板维护列表改为显示标题和参考图数量，模板引用下拉框改用模板标题。
- 支持把图片拖到主工作台提示词区域或“参考图”按钮添加参考图；参考图仍按内容哈希去重。
- 删除模板后会检查其它模板、历史任务和请求文件的引用，只有无人引用的参考图才会移入系统回收站。

### v0.2.35

- 模板维护新增 ZIP 导入，可直接读取 Image Forge 导出的提示词和参考图。
- 模板包新增版本化 `manifest.json`，图片使用 SHA-256 文件名并进行完整性校验。
- 导入模板重新分配数字 ID，内容和参考图完全相同的模板自动跳过。
- 兼容读取 `0.2.34` 导出的 Markdown 模板包，并限制文件数量、压缩包大小和解压后体积。

### v0.2.34

- 调整引用模板弹窗页脚，把对话模型、AI 填充和引用模板操作统一靠右排列。
- 模板维护新增 ZIP 导出，Markdown 清单会引用压缩包内去重后的参考图。
- 历史、模板、API 源和参考图移除统一增加删除确认。
- 同步 README 与技术设计，补齐参考图生命周期、模板导出和当前服务层说明。

### v0.2.22

- 清理已经移除的图库、设置弹窗和队列旧命令残留，收敛当前代码结构。
- 为前端工作台控制器、Rust 命令层、存储层、队列服务和发布脚本补充关键方法注释。
- 更新 README 与技术设计文档，确保功能、目录和本地数据结构描述与现有实现一致。

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
