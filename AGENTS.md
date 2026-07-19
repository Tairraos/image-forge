# 代理工作说明

- 本仓库所有回复尽量使用中文，尤其是最终交付说明、提交说明和文档类内容。
- 每次开发会话结束前，必须升级应用 patch 版本、编译，并通过 prerelease 生成 `.app`；不再要求生成 `.dmg`。
- 默认发布流程：`pnpm run patch -- <next-version>`、`pnpm build`、`cargo check --manifest-path src-tauri/Cargo.toml`、`pnpm run prerelease`。
- prerelease 成功后，`release/` 中旧文件全部移入系统回收站，只保留当前带版本号的 `.app`。
- 每次开发会话结束前必须提交 Git。
- Git 提交信息必须使用 Conventional Commits 前缀，并使用中文描述，例如 `feat: 调整生图参数面板`。
- 如果同一次会话包含多个独立任务，必须按任务拆分提交；每个任务单独提交一次，不要把无关改动混在同一个 commit 里。

## 删除与回收站规则

- 具备完全磁盘访问能力不等于可以任意删除文件。除以下目录外，任何删除动作都必须先获得用户二次许可：
  - `~/.image-forge`
  - Tauri `app_data_dir()` 返回的目录
  - `~/Workspaces/Tools/image-forge`
- 允许删除的范围也必须优先使用系统回收站，不能直接 `rm -rf` 或永久删除。
- 回收站不可用时，不执行必要删除；向用户说明原因，并提供由用户自行执行的删除命令。
- 每轮会话结束时，最终说明必须明确报告实际移入回收站或删除的内容；没有执行删除也要明确说明。
