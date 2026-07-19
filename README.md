<h1 align="center">Image Forge</h1>

<p align="center">
  <sub>一个基于 Tauri 2 + Vue 3 + Rust 的本地 AI 生图工作台 · Multi-Protocol Images · Queue · Templates · Skills</sub>
</p>

![Image Forge 1.0.1 运行界面](docs/image-forge-running.png)

<p align="center">
  <img alt="version" src="https://img.shields.io/badge/version-1.0.31-9B7BEE?style=flat-square">
  <img alt="platform desktop" src="https://img.shields.io/badge/platform-desktop-111827?style=flat-square">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white">
  <img alt="Vue" src="https://img.shields.io/badge/Vue-3-42B883?style=flat-square&logo=vuedotjs&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-backend-B7410E?style=flat-square&logo=rust&logoColor=white">
  <img alt="Images API" src="https://img.shields.io/badge/API-Images-F97316?style=flat-square">
</p>

## 简介

Image Forge 是一个本地桌面 AI 图像工作台，提供默认的“绘画”和可切换的“Agent”两种模式。绘画模式负责直接生图、队列和模板；Agent 模式以对话模型为主，负责普通聊天、受控安装/使用 Skill，并把结构化绘图任务提交到绘画队列。

它不是又一个只会转发提示词的薄壳：队列、参考图去重、协议分发、模板资产和 Skill 工作流都在本机真正跑起来。它不依赖 Python WebUI，也不把数据丢给外部数据库；除调用用户配置的模型 API 外，核心数据始终掌握在自己手里。

## 功能

- 多协议生图：按 API 源类型分别调用 GPT、Gemini、Grok、Seedream 的正确生成/编辑协议，不再假设所有模型完全兼容 OpenAI Images；
- 多模型管理：支持 `生图模型 - GPT/Gemini/Grok/Seedream` 和 `对话模型` 五种类型、独立代理、模型列表获取及 JSON 批量导入导出；
- 生图历史：任务入队、后台执行、刷新、重试、删除、运行状态轮询和失败自动重试，历史卡片显示真实图片尺寸；
- 结果预览：选择历史任务后预览输出图片，支持复制、下载到 Downloads 和在 Finder 中定位；
- 提示词模板：支持增删查改、参考图、单张效果图、从历史任务快速建模、流式 AI 填充 `{}` 占位和引用到提示词光标位置；
- Agent 与 Skill：Agent 工作区支持会话持久化、流式聊天、参考图附件、Skill 列表与本地包安装；安装前审查脚本、系统命令、符号链接和未知能力。Skill 内容及同包 `references/*.md` 只能作为 Agent 上下文，模型明确返回结构化绘图计划后由 Rust 校验并创建任务组。绘画模式不解析 `@skill`，Skill 必须在 Agent 模式使用；
- 模板包导入导出：ZIP 同时包含版本化清单、可读 Markdown 和哈希图片，可直接导回并跳过重复模板；
- 参考图工作流：支持文件选择、剪贴板图片、Finder 文件复制/剪切粘贴和文件拖放，按 SHA-256 去重持久化，历史重用和模板引用会恢复参考图；
- 安全删除：历史、模板和 API 源删除前需要确认；参考图可直接从当前草稿移除；生成图及无人引用的参考图进入系统回收站；
- 本地持久化：设置、队列、历史、请求、模板、Skill 包、生成图和参考图都保存在 `~/.image-forge`；
- 马卡龙偏紫界面：三栏工作台、小型按钮、可拖拽调整 panel 宽度，默认把弹性空间留给结果预览。

## 界面结构

Image Forge 当前是单页三栏布局：

- 左侧：生成历史记录；
- 中间：任务状态、结果预览、详情和重用；
- 右侧：生成参数、生图模型、对话模型、参考图、提示词输入和引用模板；
- 顶部：品牌、绘画/Agent 模式切换、API 源、模板、Skill 和关于入口；运行数、排队数和提示信息显示在底部状态栏。

## 开发

详见设计文档
```text
docs/technical-design.md
```

## 版本记录

版本记录只保留影响产品能力、数据兼容或开发流程的里程碑，零散的样式微调和内部重构不再单独列出。

### v1.0.18

- 关于弹窗新增“清理”入口，可扫描 `outputs/`、`references/`、`requests/` 和 `clipboard/` 中未被数据引用的孤岛文件，确认后移入系统回收站，取消不会修改文件。
- Skill 新增、编辑、URL 提取和本地包导入统一经过安全审查并生成 `manifest.json`；旧版无 manifest 的 Skill 首次读取时会安全迁移，审查失败则保留原包并显示具体原因。
- Agent Skill 回复统一支持 `chat`、`needs_input`、`ready`、`rejected` 状态；问题会持久化到会话，完整图片计划由 Rust 转成受校验的任务调用，非原生 tools 模型使用同一 JSON 协议并在结构错误时自动修复一次。
- Agent 工具进度事件现在携带真实调用标识；失败、停止和重试会重新读取会话状态，参考图附件增加本轮是否使用的明确开关。
- Agent 多图任务组改为 staged transaction 原子提交，写入失败会恢复历史、队列和请求文件，应用重启会回滚未提交事务；任务组卡片支持整组取消、重试失败项和状态查询。
- Agent 任务组卡片会根据真实队列状态自动刷新，显示等待、进行中、完成、失败或取消，并按状态禁用无效的取消/重试操作。
- Skill 的 URL/GitHub 安装现在会把 `references/` 下的 Markdown 和图片一并下载到临时包，再经过同一套 manifest 安全审查后安装；GitHub tree 地址会走目录读取流程而不是只提取单个 Markdown。

### v1.0.17

- 应用数据根目录迁移到 `~/.image-forge`，设置、队列、历史、请求、模板、生成图、参考图和剪贴板资源统一存放，并使用仅当前用户可读写的目录权限。
- Skill 按 `skills/<skill-name>/SKILL.md` 保存，支持拖入包含 `SKILL.md` 的 Skill 目录，并在执行时加载同包 `references/*.md`；`skills.json` 改为只保存索引元数据。

### v1.0.16

- UI调整，让队列，预览面板和工作台面板使用更舒适。
- 主工作台任务历史新增“今天 / 所有”时间范围筛选，默认只显示本地日期为今天的任务，并支持跨午夜自动更新。
- 运行日志改为仅保存在当前 App 进程内，使用中国时区和紧凑格式展示，并隐藏本地数据路径；代理与错误信息仅在存在时显示。
- 关于弹窗改为无边框的三行文本信息，编译时间统一显示为中国时区 UTC+8。
- 新增结构化运行日志，覆盖数据文件读写、模型列表获取、生图和对话模型调用，并记录时间、操作、结果、参数、代理状态及错误信息。
- 关于弹窗精简为版本、编译时间和开发者信息，并新增独立的运行日志查看弹窗。
- 优化引用模板弹窗的模板选择提示和页脚布局，让对话模型选择紧邻 AI 填充操作。
- 主工作台模型下拉菜单支持完整展示长标题。
- 所有参考图入口及模板效果图入口新增右键粘贴剪贴板图片。
- 重新整理引用模板弹窗：顶部为模板选择与搜索，页脚为对话模型选择和操作按钮。
- 新增 prerelease 流程，只生成带版本号的 `.app`，成功后清理 `release/` 旧产物。
- 历史任务预览区新增“建模”，可把任务提示词、参考图和生成图直接转换为模板草稿。
- 主工作台并列显示生图模型和对话模型；调整模板编辑媒体布局及引用模板搜索/模型选择位置。
- 模板新增单张效果图，支持维护页预览、引用页缩略图查看，以及随模板 ZIP 导入导出。
- 引用模板弹窗收窄到 800px；流式对话模型的 AI 填充内容会实时显示，最终完成后再整体替换。
- 新增 Skill 处理流程设计文档，完整记录当前实现、已识别的问题和分阶段的新流程方案。

### v1.0.1

- Skill 执行结果必须明确说明参考图是否需要参与提示词；生成的单图或多图提示词会自动进入受 API 源并发数控制的画图队列。
- Skill 执行弹窗保留最终提示词输出；API 源卡片在并发数大于 1 时显示并发信息。
- Skill 维护列表按名称、备注、来源、操作分列，固定名称、来源和操作列宽。
- 新增 Skill 备注、Markdown 拖放导入、`@` 补全和引用 Skill 弹窗；品牌区域支持使用自定义标题图片。
- 生图 API 源扩展为 GPT、Gemini、Grok、Seedream 四种协议和对话模型，生成、参考图编辑与模型列表按类型使用对应请求结构。
- 新增 Markdown Skill 包管理、URL/GitHub 提取、名称自动识别、同包 references 文档加载和脚本依赖拦截。
- 新增 Skill 调用工作流：对话模型读取 Skill 和用户任务后生成最终生图提示词。
- 模板维护支持持久化排序、标题查看、提示词悬浮预览和参考图缩略图预览。
- 全局滚动条改为按需显示的细样式，减少滚动区域对内容布局的占用。
- 支持从 macOS Finder 剪贴板读取图片文件引用并直接粘贴为参考图。
- 参考图在历史任务和模板引用之间持久化复用。

### v0.2.35

- 模板支持 ZIP 导入导出，包含版本化清单、哈希图片和完整性校验。
- 重复模板和重复参考图会自动跳过，删除资源前会检查引用关系。
- 前端拆分为 Vue 组件，Rust 后端按模型、存储、命令和服务分层。
- API 源支持生图模型与对话模型，新增技术设计文档和版本化发布流程。

### v0.2.0

- 确立三栏工作台、可拖拽 panel、Images API 和本地 JSON 数据存储。
- 补齐中文 README、Vue 调试方式和 Tauri 发布产物规则。

### v0.1.0

- 初始化 Tauri 版 Image Forge，建立 Vue 3 + Naive UI 前端与 Rust 后端。
- 实现 API 源管理、生图队列、历史、结果预览和提示词模板基础能力。


### 感谢
本项目参考了以下开源项目的设计理念：https://github.com/kadevin/ilab-gpt-conjure
