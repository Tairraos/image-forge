<h1 align="center">Image Forge</h1>

<p align="center">
  <sub>一个基于 Tauri 2 + Vue 3 + Rust 的本地 AI 生图工作台 · Multi-Protocol Images · Queue · Templates · Skills</sub>
</p>

![Image Forge 1.0.1 运行界面](docs/image-forge-1.0.1-running.png)

<p align="center">
  <img alt="version" src="https://img.shields.io/badge/version-1.0.3-9B7BEE?style=flat-square">
  <img alt="platform desktop" src="https://img.shields.io/badge/platform-desktop-111827?style=flat-square">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white">
  <img alt="Vue" src="https://img.shields.io/badge/Vue-3-42B883?style=flat-square&logo=vuedotjs&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-backend-B7410E?style=flat-square&logo=rust&logoColor=white">
  <img alt="Images API" src="https://img.shields.io/badge/API-Images-F97316?style=flat-square">
</p>

## 简介

Image Forge 是一个本地桌面生图工作台。它把 GPT、Gemini、Grok、Seedream 四套并不完全兼容的生图协议收进同一个工作台，同时管理 API 源、生成历史、结果预览、提示词模板和纯 Markdown Skill。

它不是又一个只会转发提示词的薄壳：队列、参考图去重、协议分发、模板资产和 Skill 工作流都在本机真正跑起来。它不依赖 Python WebUI，也不把数据丢给外部数据库；除调用用户配置的模型 API 外，核心数据始终掌握在自己手里。

## 功能

- 多协议生图：按 API 源类型分别调用 GPT、Gemini、Grok、Seedream 的正确生成/编辑协议，不再假设所有模型完全兼容 OpenAI Images；
- 多模型管理：支持 `生图模型 - GPT/Gemini/Grok/Seedream` 和 `对话模型` 五种类型、独立代理、模型列表获取及 JSON 批量导入导出；
- 生图历史：任务入队、后台执行、刷新、重试、删除、运行状态轮询和失败自动重试，历史卡片显示真实图片尺寸；
- 结果预览：选择历史任务后预览输出图片，支持复制、下载到 Downloads 和在 Finder 中定位；
- 提示词模板：支持增删查改、参考图、单张效果图、工作台快捷保存、流式 AI 填充 `{}` 占位和引用到提示词光标位置；
- 纯 Markdown Skill：支持手动录入或从 URL/GitHub 提取，自动识别名称并拒绝脚本依赖；可把 Skill 与画面需求交给对话模型，生成最终生图提示词；
- 模板包导入导出：ZIP 同时包含版本化清单、可读 Markdown 和哈希图片，可直接导回并跳过重复模板；
- 参考图工作流：支持文件选择、剪贴板图片、Finder 文件复制/剪切粘贴和文件拖放，按 SHA-256 去重持久化，历史重用和模板引用会恢复参考图；
- 安全删除：历史、模板和 API 源删除前需要确认；参考图可直接从当前草稿移除；生成图及无人引用的参考图进入系统回收站；
- 本地持久化：设置、队列、历史、请求、模板、Skill 和参考图都保存在本机应用数据目录；
- 马卡龙偏紫界面：三栏工作台、小型按钮、可拖拽调整 panel 宽度，默认把弹性空间留给结果预览。

## 界面结构

Image Forge 当前是单页三栏布局：

- 左侧：生成历史记录；
- 中间：任务状态、结果预览、详情和重用；
- 右侧：生成参数、生图模型、参考图、提示词输入、存为模板、引用模板和使用 Skill；
- 顶部：品牌、API 源、模板、Skill 和关于入口；运行数、排队数和提示信息显示在底部状态栏。

## 数据与架构

项目没有使用传统数据库，而是用 Tauri 应用数据目录里的 JSON 文件和图片文件组成轻量本地数据层：

```text
app_data_dir/
  settings.json
  queue.json
  history.json
  prompt-templates.json
  skills.json
  requests/
  outputs/
  references/
  clipboard/
```

`references/` 保存任务和模板共享的参考图资源，文件名使用内容哈希；`skills.json` 保存不依赖脚本的纯 Markdown Skill；`clipboard/` 仅作为旧版兼容目录保留。

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
- `src-tauri/src/services/`：多协议 Images、Chat/Models API、Skill URL 提取、后台队列、剪贴板、参考图资源和模板 ZIP 导入导出；
- `src-tauri/src/lib.rs`：Tauri 入口和命令注册；
- `src-tauri/tauri.conf.json`：Tauri 窗口、打包、权限和应用版本配置；
- `docs/technical-design.md`：技术架构、数据流、存储结构和运行逻辑说明。
- `docs/skill-process.md`：当前 Skill 录入、规划、交互、入队流程，以及建议的新流程设计。

## 调整 UI 的入口

- 改三栏默认宽度：修改 `src/App.vue` 里的 `panelSizes` 和 `workspaceStyle`；
- 改拖拽范围：修改 `src/App.vue` 的 `startPanelResize()`；
- 改顶部 API 源/模板/Skill 入口：修改 `src/components/AppTopbar.vue`；
- 改 API 源/模型管理：修改 `src/components/dialogs/ApiSourceDialog.vue`；
- 改工作台参数区和生图模型选择：修改 `src/components/ComposerPanel.vue`；
- 改队列/结果预览：修改 `src/components/QueuePanel.vue` 和 `src/components/ResultPanel.vue`；
- 改模板维护：修改 `src/components/dialogs/TemplateManagerDialog.vue`；列表支持手动排序、标题点击查看、提示词/参考图悬浮预览和编辑删除；
- 改模板引用和 AI 填充：修改 `src/components/dialogs/TemplateReferenceDialog.vue` 和 `src-tauri/src/services/chat.rs`；
- 改 Skill 管理和调用：修改 `src/components/dialogs/SkillManagerDialog.vue`、`SkillEditorDialog.vue`、`SkillReferenceDialog.vue` 以及 `src-tauri/src/services/skill.rs`；
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

版本记录只保留影响产品能力、数据兼容或开发流程的里程碑，零散的样式微调和内部重构不再单独列出。

### v1.0.3

- 模板新增单张效果图，支持维护页预览、引用页缩略图查看，以及随模板 ZIP 导入导出。
- 引用模板弹窗收窄到 800px；流式对话模型的 AI 填充内容会实时显示，最终完成后再整体替换。

### v1.0.2

- 新增 Skill 处理流程设计文档，完整记录当前实现、已识别的问题和分阶段的新流程方案。

### v1.0.1

- Skill 执行结果必须明确说明参考图是否需要参与提示词；生成的单图或多图提示词会自动进入受 API 源并发数控制的画图队列。
- Skill 执行弹窗保留最终提示词输出；API 源卡片在并发数大于 1 时显示并发信息。
- Skill 维护列表按名称、备注、来源、操作分列，固定名称、来源和操作列宽。
- 新增 Skill 备注、Markdown 拖放导入、`@` 补全和引用 Skill 弹窗；品牌区域支持使用自定义标题图片。

### v0.2.52

- 生图 API 源扩展为 GPT、Gemini、Grok、Seedream 四种协议和对话模型，生成、参考图编辑与模型列表按类型使用对应请求结构。
- 新增纯 Markdown Skill 管理、URL/GitHub 提取、名称自动识别和脚本依赖拦截。
- 新增 Skill 调用工作流：对话模型读取 Skill 和用户任务后生成最终生图提示词。

### v0.2.47

- 模板维护支持持久化排序、标题查看、提示词悬浮预览和参考图缩略图预览。
- 全局滚动条改为按需显示的细样式，减少滚动区域对内容布局的占用。

### v0.2.41

- 支持从 macOS Finder 剪贴板读取图片文件引用并直接粘贴为参考图。
- 参考图在历史任务和模板引用之间持久化复用。

### v0.2.35

- 模板支持 ZIP 导入导出，包含版本化清单、哈希图片和完整性校验。
- 重复模板和重复参考图会自动跳过，删除资源前会检查引用关系。

### v0.2.5

- 前端拆分为 Vue 组件，Rust 后端按模型、存储、命令和服务分层。
- API 源支持生图模型与对话模型，新增技术设计文档和版本化发布流程。

### v0.2.0

- 确立三栏工作台、可拖拽 panel、Images API 和本地 JSON 数据存储。
- 补齐中文 README、Vue 调试方式和 Tauri 发布产物规则。

### v0.1.0

- 初始化 Tauri 版 Image Forge，建立 Vue 3 + Naive UI 前端与 Rust 后端。
- 实现 API 源管理、生图队列、历史、结果预览和提示词模板基础能力。
