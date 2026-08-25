# Golden Principles

这是 Image Forge 长期要守住的规则。每条原则尽量标注「自动检查」还是「人工判断」，优先把能自动化的做成 lint / 测试 / 脚本。

## 1. 复用现有能力，不重复实现

双平台已经通过 `src/api/` 适配器层统一。新增一个能力时，不要在一个平台写死实现、另一个平台再抄一份。

- 桌面版：命令在 `commands.rs`，SQLite 在 `history_db.rs`，业务在 `services/`。
- Web 版：命令实现在 `adapter-web.js`，IndexedDB 在 `db.js`，业务在 `src/api/` 其余文件。
- 前端组件**只通过 `api.*` 调用**，不直接 `invoke()` 或 `fetch()`。

**自动检查**：`src/api/index.js` 是唯一导出入口。新增 adapter 函数必须同步更新 `index.js` 导出列表和两个 adapter 文件。此约束由 `tests/api/adapter-consistency.spec.js` 验证（见「验证」）。

## 2. 保持模块边界，UI 不持有业务状态

`src/App.vue` 是唯一业务状态源。子组件通过 props 接收状态、通过 events 把动作交回。不要在展示组件里直接写业务状态或调用 API。

**自动检查**：`vue/no-mutating-props`（ESLint）已拦截直接改 prop；`no-unused-vars` 提醒未使用的注入。

## 3. 正确失败，不让界面无限等待

网络错误、JSON 解析错误、工具调用错误、生图失败，都必须落到可见状态（任务 failed / 消息 error / 状态栏），不能让 UI 卡在「运行中」。

**自动检查**：`no-empty`（ESLint）禁止空 catch 块，强制每条错误路径有处理。

## 4. 保护数据边界

- 删除文件优先走系统回收站，不 `rm -rf`。
- 导出数据包里的 API Key 是明文，导出文件必须保存在可信位置（已有 UI 提示）。
- 图片按 SHA-256 内容去重，任务 / 模板 / 会话共享同一份资源引用。
- Web 版图片字节永不进 localStorage（已由 `blob.js` 落到 `~/.image-forge/` 或 Vercel Blob）。

**自动检查**：`blob.js` 的上传/回退分支有单测覆盖；删除规则见 `AGENTS.md` 硬约束。

## 5. 配置归一化，不散落

`settings.providers` 是统一配置列表。`modelType` 取 `image-gpt` / `image-gemini` / `image-grok` / `chat`；旧值 `image` 或未知类型在读取时归一化，不要把厂商差异散落到组件。

**自动检查**：`src/lib/models.js` 的解析与归一化有单测。

## 6. 关键功能必须有验证

- 新增 adapter 函数 → 同步 `index.js` + 两个 adapter + 一个测试。
- 新增 SQLite / IndexedDB 表 → 同步 `history_db.rs` 和 `db.js` + 迁移逻辑。
- 新增模型协议 → 同步 provider 归一化、模型列表、请求组装、响应解析和测试。
- 新增 Agent 工具 → 同步 schema、权限边界、降级 envelope 和集成测试。

**自动检查**：见下方「验证」——`scripts/verify.mjs` 会跑 lint + 格式 + 测试 + Rust check。

## 7. 修改后必须跑检查

任何代码改动完成后，先跑 `pnpm verify`，失败就修复，不要带着红字提交。

**自动检查**：`scripts/verify.mjs` 一次跑完主要检查；`husky` pre-commit 会拦下未格式化的文件。

## 验证清单（机器可执行）

| 检查 | 命令 | 说明 |
| --- | --- | --- |
| 适配器导出完整性 | `pnpm test -- tests/api/adapter-consistency.spec.js` | 确保 `index.js` 导出的函数在两个 adapter 都存在 |
| 适配器层逻辑 | `pnpm test -- tests/api/` | db / blob / queue 的核心逻辑 |
| 全部测试 | `pnpm test` | Vitest 全套 |
| 静态检查 | `pnpm lint` | ESLint |
| 格式 | `pnpm fmt:check` | Prettier |
| Rust | `cargo check --manifest-path src-tauri/Cargo.toml` | 类型检查 |
| 一键全检 | `pnpm verify` | 上面几项串起来 |

## 仍只能人工判断的部分

- 架构是否清晰、模块是否越层（代码评审层面，无法完全靠 lint）。
- UI 交互是否符合预期（需要启动应用人工点按）。
- 生图结果质量（依赖真实 API 调用）。
- 数据迁移是否安全（涉及真实用户数据，需谨慎）。
