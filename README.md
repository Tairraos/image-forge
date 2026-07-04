# Image Forge

Image Forge 是一个基于 Tauri 2、Vue 3、Naive UI 和 Rust 的本地生图工作台。项目完全使用原生 Tauri 前后端实现，不依赖 Python WebUI。

## 功能

- OpenAI 兼容 Images API：支持 `/images/generations` 文生图和 `/images/edits` 参考图编辑。
- 多 API 源管理：可维护多个 Base URL、API Key、图像模型、并发数和备注，并选择当前默认源。
- 生图队列：任务入队、后台执行、取消、重试、移到队首、运行状态轮询和失败自动重试。
- 结果预览：选择队列或历史任务后预览输出图片，支持在 Finder 中定位和保存到图库。
- 图库管理：导入本地图片、保存生成结果、编辑名称/分类/备注，并作为参考图复用。
- 提示词模板：保存常用模板，支持插入、替换、收藏字段和使用次数记录。
- 提示词片段：维护短标签片段，快速插入到当前提示词。
- 参考图工作流：支持多张参考图，自动生成本地预览，并可从图库继续添加。
- 本地持久化：设置、队列、历史、图库、模板和片段写入应用数据目录。
- 马卡龙偏紫界面：三栏工作台、小型按钮、可拖拽调整 panel 宽度，默认把弹性空间留给结果预览。

## 开发

```bash
pnpm install
pnpm tauri dev
```

只查看 Vue 前端界面：

```bash
pnpm run dev
```

Vite 调试服务默认运行在 `http://127.0.0.1:1421`。这个方式适合调样式和布局，但浏览器里没有 Tauri 桌面运行时，所以调用本地文件、弹窗、队列命令等桌面功能会报错。需要完整功能调试时使用 `pnpm tauri dev`。

常用检查：

```bash
pnpm build
cd src-tauri
cargo fmt --check
cargo check
cargo test
```

打正式包：

```bash
pnpm tauri build
```

每个版本的 `.dmg` 和 `.app` 都复制到 `release/` 目录，文件名必须带版本号。`release/` 不提交进 Git。

打包调试版本：

```bash
pnpm tauri build --debug
```

## 目录

- `src/App.vue`：主界面、状态管理、队列操作、弹窗和 API 源管理。
- `src/styles.css`：整体布局、马卡龙紫配色、panel 尺寸、拖拽条和响应式细节。
- `src/tauri.js`：前端调用 Tauri 命令与文件协议转换的轻封装。
- `src/assets/`：应用图标资源。
- `src-tauri/src/lib.rs`：Rust 后端命令、Images API 调用、队列执行、文件持久化。
- `src-tauri/tauri.conf.json`：Tauri 窗口、打包、权限和应用版本配置。

## 调整 UI 的入口

- 改三栏默认宽度：修改 `src/App.vue` 里的 `panelSizes`，以及 `workspaceStyle` 的 `gridTemplateColumns`。
- 改拖拽范围：修改 `src/App.vue` 的 `startPanelResize()`，里面有队列、结果预览和工作台的最小/最大宽度约束。
- 改 panel 位置：调整 `src/App.vue` 模板中 `.queue-column`、`.result-column`、`.composer-column` 的顺序，同时保持中间的 `.panel-resizer`。
- 改工作台内部顺序：调整 `src/App.vue` 中 `.composer-column` 里的 `control-surface`、`prompt-input`、`quick-bar`、`reference-strip`。
- 改配色：优先修改 `src/App.vue` 的 `themeOverrides`，再修改 `src/styles.css` 里的背景、边框、文字和状态色。
- 改 panel 弹性和滚动：修改 `src/styles.css` 里的 `.workspace`、`.queue-column`、`.result-column`、`.composer-column`、`.output-grid`、`.task-stack`。
- 改窗口最小尺寸：修改 `src-tauri/tauri.conf.json` 的 `minWidth` 和 `minHeight`。

## Git 与版本规则

- 本项目使用 PNPM 管理前端依赖。
- 代码提交遵循 Conventional Commits，例如 `feat: 增加队列工作台`。
- 以后每次会话提交代码时都需要升级版本，并把版本差异写入本 README。
- 每个版本都需要产出 `.dmg` 和 `.app` 到 `release/`，但不要提交这些二进制产物。

## 版本记录

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
