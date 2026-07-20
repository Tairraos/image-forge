<h1 align="center">Image Forge</h1>

<p align="center">
  <strong>把灵感变成可管理、可复用、可持续迭代的视觉资产。</strong>
</p>

<p align="center">
  本地优先的 AI 图像生产工作台 · 绘画队列 · Agent 编排 · Skill 工作流
</p>

![Image Forge 运行界面](docs/image-forge-running.png)

<p align="center">
  <img alt="version" src="https://img.shields.io/badge/version-1.0.47-9B7BEE?style=flat-square">
  <img alt="platform" src="https://img.shields.io/badge/platform-macOS-111827?style=flat-square">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white">
  <img alt="Vue" src="https://img.shields.io/badge/Vue-3-42B883?style=flat-square&logo=vuedotjs&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-backend-B7410E?style=flat-square&logo=rust&logoColor=white">
</p>

## 这是什么

Image Forge 不是一个把提示词转发给接口的薄壳，而是一套本地运行的视觉生产系统：它把提示词、参考图、模型参数、队列状态、生成结果和复用关系组织成一条完整链路。

你可以在绘画模式里直接控制画面，也可以在 Agent 模式里用自然语言描述目标，让 Agent 分析需求、调用受控 Skill、生成结构化绘图计划，再把任务交给同一套可靠的绘画队列。模型负责理解与规划，Rust 负责校验与执行，重要数据留在本机。

## 核心体验

### 绘画模式：把一次生成做成可追踪的任务

- 多协议生图：GPT Images、Gemini、Grok 和 Seedream 使用各自正确的请求格式、鉴权方式与参考图协议。
- 三栏工作台：左侧历史队列、中间状态与预览、右侧参数与提示词，面板宽度可以按工作习惯调整。
- 稳定队列：支持排队、运行、完成、失败、取消、重试、并发限制和异常重启恢复。
- 结果资产化：生成图落盘后可复制、下载、在 Finder 中定位，并从历史任务重新建立模板。
- 参数保持克制：分辨率、比例、质量、生图模型和提示词模式足够表达意图，数量与输出格式由应用统一控制。

### Agent 模式：让对话成为生产入口

- 持久化会话：每个会话保存消息、时间、模型、附件、Tool Call、Skill 关联和绘图任务组。
- Markdown 原生渲染：AI 的标题、列表、引用、代码块和链接在对话中按 Markdown 展示。
- 参考图优先：支持选择、粘贴、右键粘贴和拖放；只要存在参考图，绘画计划默认会把它纳入任务。
- 直接绘画：勾选“直接绘画”后，提示词绕过对话模型直接进入生图模型；默认回车发送，Shift+Enter 换行。
- 受控工具：Skill 搜索、安装、使用、绘图任务创建和任务状态查询均经过 Rust 参数校验。
- 任务组联动：Agent 创建的单图或多图任务会进入绘画队列，消息中的任务卡片可以直接跳转到绘画结果。

### Skill：可扩展，但不放弃边界

Skill 是 Markdown 包，不是脚本插件。安装前会检查入口文件、路径、符号链接、能力声明、脚本目录、可执行扩展名和危险指令；通过审查后才会原子安装到本地包目录。

Skill 可以提供聊天规范、图片规划规范和参考图规范，但不能获得终端、任意文件系统、任意网络、浏览器或数据库权限。它的参考 Markdown 会作为上下文参与推理，不会被当作可执行程序。

## 你会得到什么

- **可复用的提示词系统**：模板支持标题、内容、参考图、效果图、排序、使用次数、ZIP 导入导出和 AI 填充 `{}` 占位符。
- **一致的参考图资产**：图片按 SHA-256 内容去重，任务、模板和 Agent 会话共享同一份本地资源。
- **可恢复的本地数据**：设置、队列、历史、请求、输出、模板、Skill 和会话都是可读的本地文件。
- **不被厂商协议绑架**：模型类型是调用行为的一部分，协议差异被封装在 Rust 服务层，而不是散落在界面代码里。
- **适合长时间工作的桌面界面**：最小窗口尺寸为 `1200×800`，窗口尺寸按逻辑像素保存，Retina 屏幕恢复不会缩成半个窗口。

## 界面入口

顶部工作区切换器提供两个入口：

- `绘画`：直接生图、查看历史、管理队列和编辑参数；绘画模式不会解析 `@skill`。
- `Agent`：聊天、Skill、参考图和绘画任务编排；没有会话时会自动创建新对话。

全局入口还包括 API 源、模板、Skill 和关于。Agent 左侧只展示会话历史，不把 Skill 混入会话列表；会话按创建时间保持稳定顺序，卡片显示标题和时间。

## 本地数据

默认数据目录为 `~/.image-forge`：

```text
~/.image-forge/
  settings.json              # API 源、默认模型和工作区设置
  queue.json                 # waiting / running 队列状态
  history.json               # 绘画历史任务
  prompt-templates.json      # 提示词模板
  skills.json                # Skill 索引
  agent/sessions/            # Agent 会话
  requests/                  # 可重试的原始绘图请求
  outputs/                   # 生成图片
  references/                # SHA-256 去重后的参考图
  skills/                    # Markdown Skill 包
```

除调用你配置的模型 API 外，应用不依赖远程数据库。清理孤岛文件时会扫描历史、模板、请求和会话引用；无人引用的图片进入系统回收站，而不是静默永久删除。

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

完整的模块边界、数据流、队列事务、Agent 协议、Skill 安全门和发布规则见 [技术设计](docs/technical-design.md)。

## 许可证与致谢

项目的许可证信息以仓库实际文件为准。界面与工作流设计参考了开源项目 [ilab-gpt-conjure](https://github.com/kadevin/ilab-gpt-conjure) 的部分理念。
