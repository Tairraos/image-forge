# AGENTS.md — 项目导航

> 这是 Image Forge 的入口导航，给 Agent / Codex 快速定位。长期知识在 `docs/` 下，能自动检查的规则已转为脚本 / lint / 测试，不要在这里堆细节。

<!-- CODEGRAPH_START -->

## CodeGraph

In repositories indexed by CodeGraph (a `.codegraph/` directory exists at the repo root), reach for it BEFORE grep/find or reading files when you need to understand or locate code:

- **MCP tool** (when available): `codegraph_explore` answers most code questions in one call — the relevant symbols' verbatim source plus the call paths between them, including dynamic-dispatch hops grep can't follow. Name a file or symbol in the query to read its current line-numbered source. If it's listed but deferred, load it by name via tool search.
- **Shell** (always works): `codegraph explore "<symbol names or question>"` prints the same output.

If there is no `.codegraph/` directory, skip CodeGraph entirely — indexing is the user's decision.
<!-- CODEGRAPH_END -->

## 项目是什么

本地优先的 AI 图像生产工作台：Tauri 2 + Vue 3 桌面应用，同一套前端代码可编译为纯 Web 版（Vite + IndexedDB + Vercel Blob）。模型负责理解与规划，Rust 负责校验与执行，数据留在本机。

## 关键目录

| 目录 | 作用 |
| --- | --- |
| `src/App.vue` | 前端唯一业务状态源，组件通过 props/events 协作 |
| `src/api/` | 适配器层：`index.js` 运行时检测，自动切 Tauri / Web |
| `src/components/` | Vue 组件（`dialogs/` 是对话框，`snippets/` 是微型组件） |
| `src/lib/` | 纯函数工具（格式化、模型解析、主题等） |
| `src-tauri/src/` | Rust 后端：`commands.rs` 命令层、`services/` 业务层、`store.rs` + `history_db.rs` SQLite |
| `scripts/` | 构建 / 发布 / 数据同步 / 验证脚本 |
| `tests/` | Vitest 测试（`api/` 适配器层、`lib/` 工具、`components/` 组件） |
| `docs/` | 架构、黄金原则、开发指南、生图 API 参考 |

## 快速开始

```bash
pnpm install        # 安装依赖
pnpm tauri dev      # 桌面完整开发（Tauri 命令、队列、本地图片协议可用）
pnpm dev            # 仅 Web 开发（http://localhost:1421，Tauri 能力不可用）
```

## 验证与发布

```bash
pnpm verify         # 完成任务后必跑：lint + 格式 + 测试 + Rust check（见 scripts/verify.mjs）
pnpm check          # 同上，轻量版（不含 fmt:check 和 cargo test）
pnpm ship <version> # 一键发布：patch + build + cargo check + prerelease
```

## 关键规则在哪

- **黄金原则**（复用、边界、错误处理、数据、验证、发布）→ `docs/golden-principles.md`
- **架构与模块边界** → `docs/technical-design.md`
- **Web 版开发与调试** → `docs/web-dev-guide.md`
- **生图 API 参数** → `docs/生图API使用说明.md`、`docs/生图参数参考.md`
- **删除与回收站硬约束** → 见下方（每次会话都要遵守）

## 遇到问题先看

1. 先读 `docs/technical-design.md` 理解当前架构与数据流
2. 再读 `docs/golden-principles.md` 理解不可违反的约束
3. 跑 `pnpm verify` 确认当前是否干净
4. 改代码后跑对应测试：`pnpm test -- tests/api/db.spec.js`

## 完成一次任务的固定流程

改代码 → `pnpm verify` → 修复失败 → `pnpm ship <version>` → 按任务拆分提交（Conventional Commits + 中文描述）

## 删除与回收站规则（硬约束）

- 除 `~/.image-forge`、Tauri `app_data_dir()`、`~/Workspaces/Tools/image-forge` 外，删除任何文件前必须先获得用户二次确认。
- 允许删除的范围也优先用系统回收站，不直接 `rm -rf`。
- 回收站不可用时，不执行必要删除；说明原因并给出用户可自行执行的命令。
- 每轮结束的交付说明必须报告实际移入回收站 / 删除的内容；没有删除也要明确说明。
