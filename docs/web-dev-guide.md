# Web 版开发指南

## 快速开始

```bash
# 安装依赖（首次）
pnpm install

# 复制环境变量
cp .env.example .env

# 启动 Web 开发服务器
pnpm dev
```

浏览器打开 `http://localhost:1421`，即可看到 Web 版界面。Vite 热更新开箱即用，改代码自动刷新。

## 调试

- 浏览器 DevTools → Console 查看日志
- DevTools → Application → IndexedDB → ImageForge 查看数据库
- DevTools → Application → Local Storage 查看设置和会话
- 网络请求直接在 Network 面板查看

## 桌面版 vs Web 版切换

同一个代码库，根据运行时环境自动切换：

| 环境 | 判定条件 | 适配器 | 数据存储 |
|---|---|---|---|
| `pnpm tauri dev` | `window.__TAURI_INTERNALS__` 存在 | `adapter-tauri.js` | `~/.image-forge/`（SQLite + 文件系统） |
| `pnpm dev`（浏览器） | `window.__TAURI_INTERNALS__` 不存在 | `adapter-web.js` | 浏览器 IndexedDB + localStorage |

**不需要任何构建配置切换**，同一个 `pnpm dev` 在浏览器打开就是 Web 版，在 Tauri 窗口打开就是桌面版。

## 常见问题

### Web 版能用 SQLite 吗？

不能。浏览器没有文件系统访问权限，无法使用 SQLite。Web 版使用 **IndexedDB**（通过 Dexie.js）作为替代：

| 桌面版 | Web 版 |
|---|---|
| SQLite（`library.sqlite`） | IndexedDB（`src/api/db.js`） |
| 文件系统（`~/.image-forge/`） | localStorage + Vercel Blob |
| 图片存本地文件 | 图片存 Vercel Blob 或 data URL |

### 能和桌面版共用 `~/.image-forge` 吗？

不能直接共用（数据存储机制不同），但提供了同步工具：

**方式一：控制台导入**
```bash
pnpm sync:web
```
复制输出的 JS 代码，粘贴到浏览器控制台（F12 → Console），回车执行。

**方式二：可视化页面导入**
```bash
pnpm sync:web:serve
```
浏览器打开 `http://localhost:1422`，点击「导入数据」按钮。

同步内容包括：API 源设置、提示词模板、Agent 会话。

### 生图 API 调用从哪里发出？

Web 版直接从浏览器调用 OpenAI/Gemini/Grok API（`src/api/providers.js`）。API Key 存在 localStorage 中。

> 注意：API Key 在浏览器中明文存储，DevTools 可见。如需保护 Key，可部署 Vercel Serverless Function 做代理。

### 密码锁在哪里配置？

编辑 `.env` 文件中的 `VITE_ACCESS_PASSWORD`。默认密码为 `image-forge`。桌面版不显示密码锁。

### 图片存在哪里？

1. 优先：Vercel Blob（需配置 `VITE_BLOB_READ_WRITE_TOKEN`）
2. 回退：data URL 存在 IndexedDB 中（不适合大量图片，会撑爆浏览器存储）

## 项目结构

```
src/api/                   # 适配层（桌面版和 Web 版共用）
  index.js                 # 环境检测，自动选择适配器
  adapter-tauri.js         # 桌面版：invoke() 封装
  adapter-web.js           # Web 版：完整 JS 实现
  db.js                    # Web 版 IndexedDB 数据库
  blob.js                  # Web 版 Vercel Blob 存储
  providers.js             # Web 版生图 API 调用
  queue.js                 # Web 版队列调度
  agent.js                 # Web 版 Agent 对话引擎
```