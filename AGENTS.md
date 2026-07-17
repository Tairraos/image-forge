# 代理工作说明

- 本仓库所有回复尽量使用中文，尤其是最终交付说明、提交说明和文档类内容。
- 每次开发会话结束前，必须升级应用 patch 版本、编译，并通过 prerelease 生成 `.app`；不再要求生成 `.dmg`。
- 默认发布流程：`pnpm run patch -- <next-version>`、`pnpm build`、`cargo check --manifest-path src-tauri/Cargo.toml`、`pnpm run prerelease`。
- prerelease 成功后，`release/` 中旧文件全部移入系统回收站，只保留当前带版本号的 `.app`。
- 每次开发会话结束前必须提交 Git。
- Git 提交信息必须使用 Conventional Commits 前缀，并使用中文描述，例如 `feat: 调整生图参数面板`。
- 如果同一次会话包含多个独立任务，必须按任务拆分提交；每个任务单独提交一次，不要把无关改动混在同一个 commit 里。
