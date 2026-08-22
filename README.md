<h1 align="center">Image Forge</h1>

<p align="center">
  <strong>把灵感变成可管理、可复用、可持续迭代的视觉资产。</strong>
</p>

<p align="center">
  本地优先的 AI 图像生产工作台 · Agent 对话绘画
</p>

![Image Forge 运行界面](docs/image-forge-running.png)

<p align="center">
  <img alt="version" src="https://img.shields.io/badge/version-1.0.75-9B7BEE?style=flat-square">
  <img alt="platform" src="https://img.shields.io/badge/platform-macOS-111827?style=flat-square">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white">
  <img alt="Vue" src="https://img.shields.io/badge/Vue-3-42B883?style=flat-square&logo=vuedotjs&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-backend-B7410E?style=flat-square&logo=rust&logoColor=white">
</p>

## 这是什么

Image Forge 不是一个把提示词转发给接口的薄壳，而是一套本地运行的视觉生产系统：它把提示词、参考图、模型参数、队列状态、生成结果和复用关系组织成一条完整链路。

在 Agent 里用自然语言描述目标，让模型分析需求、生成结构化绘图计划，再交给同一套本地绘画队列执行。模型负责理解与规划，Rust 负责校验与执行，重要数据留在本机。

## 核心体验

### Agent 模式：对话即生产入口

- 持久化会话：每个会话保存消息、时间、模型、附件、Tool Call 和绘图任务组。
- Markdown 原生渲染：AI 的标题、列表、引用、代码块和链接在对话中按 Markdown 展示。
- 参考图优先：支持选择、粘贴、右键粘贴和拖放；只要存在参考图，绘画计划默认会把它纳入任务。
- 直接绘画：勾选“直接绘画”后，提示词绕过对话模型直接进入生图模型；默认回车发送，Shift+Enter 换行。
- 受控工具：绘图任务创建和任务状态查询均经过 Rust 参数校验。
- 任务组联动：Agent 创建的单图或多图任务会进入绘画队列，消息中的任务卡片可预览结果并跟踪状态。

## 你会得到什么

- **可复用的提示词系统**：模板支持标题、内容、参考图、效果图、排序、使用次数、ZIP 导入导出和 AI 填充 `{}` 占位符。
- **一致的参考图资产**：图片按 SHA-256 内容去重，任务、模板和 Agent 会话共享同一份本地资源。
- **可恢复的本地数据**：任务历史使用本地 SQLite 事务保存，其余设置、队列、请求、输出、模板和会话仍是可读的本地文件。
- **不被厂商协议绑架**：模型类型是调用行为的一部分，协议差异被封装在 Rust 服务层，而不是散落在界面代码里。
- **适合长时间工作的桌面界面**：最小窗口尺寸为 `1200×800`，窗口尺寸按逻辑像素保存，Retina 屏幕恢复不会缩成半个窗口。

## 界面入口

应用默认进入 Agent 工作台：

- 左侧：新对话、内嵌图片库、设置，以及会话历史。
- 中间：当前对话或按月份浏览的图片库。
- 底部：当前绘图 API / 对话 API 与队列状态。

设置对话框继续提供模板库、对话 API、绘图 API 和关于。

## 本地数据

默认数据目录为 `~/.image-forge`：

```text
~/.image-forge/
  settings.json              # API 源、默认模型和工作区设置
  queue.json                 # waiting / running 队列状态
  library.sqlite             # 任务、提示词和图片索引
  prompt-templates.json      # 提示词模板
  agent/sessions/            # Agent 会话
  requests/                  # 可重试的原始绘图请求
  outputs/YYYY/MM/           # 按年月组织的生成图片
  references/                # SHA-256 去重后的参考图
```

除调用你配置的模型 API 外，应用不依赖远程数据库。首次升级会把旧 `history.json` 和对应图片迁移到 SQLite 与年月目录，并保留 `.bak` 和 `.migrated` 备份。清理孤岛文件时会扫描数据库、模板、请求和会话引用；无人引用的图片进入系统回收站，而不是静默永久删除。

## 开发

### 环境

- Node.js 与 PNPM
- Rust toolchain
- macOS 桌面环境（完整 Tauri 工作流）

安装依赖：

```bash
pnpm install
```

启动完整桌面开发模式：

```bash
pnpm tauri dev
```

只启动 Vue 开发服务器：

```bash
pnpm run dev
```

只启动前端时，Tauri 命令、系统文件对话框、队列和本地图片协议不可用；需要完整功能时使用 `pnpm tauri dev`。

### 检查

```bash
pnpm test
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

### 版本与预发布

升级 patch 版本：

```bash
pnpm run patch -- <next-version>
```

生成日常预发布 App：

```bash
pnpm run prerelease
```

预发布流程会构建、签名并在 `release/` 生成当前版本的 `.app`；日常开发不要求生成 `.dmg`。正式发布流程可按项目维护者的发布环境另行执行。

## 设计文档

完整的模块边界、数据流、队列事务、Agent 协议和发布规则见 [技术设计](docs/technical-design.md)。

## 许可证与致谢

项目的许可证信息以仓库实际文件为准。界面与工作流设计参考了开源项目 [ilab-gpt-conjure](https://github.com/kadevin/ilab-gpt-conjure) 的部分理念。
