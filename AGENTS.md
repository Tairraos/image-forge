# 代理工作说明

- 本仓库所有回复尽量使用中文，尤其是最终交付说明、提交说明和文档类内容。
- 每次开发会话结束前，必须升级应用 patch 版本、编译、打包发布产物，并确认 `release/` 下存在对应版本的 `.app` 和 `.dmg`。
- 默认发布流程：`pnpm run patch -- <next-version>`、`pnpm build`、`cargo check --manifest-path src-tauri/Cargo.toml`、`pnpm run release`。
- 每次开发会话结束前必须提交 Git。
- Git 提交信息必须使用 Conventional Commits 前缀，并使用中文描述，例如 `feat: 调整生图参数面板`。
- 如果同一次会话包含多个独立任务，必须按任务拆分提交；每个任务单独提交一次，不要把无关改动混在同一个 commit 里。
