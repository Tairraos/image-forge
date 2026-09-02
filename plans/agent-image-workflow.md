# Agent 生图闭环加固 + 模板闭环

> 本计划面向「换一个 Agent 接手」（goal 模式自主执行）编写，包含全部上下文，不需要原对话。

## 目标

1. 修复 Agent 多轮对话的历史重建缺陷（孤立 tool 消息，严格 OpenAI 兼容端点会 400）。
2. 补齐「聊天中调用模板画图」闭环：Agent 可查模板、计划可引用模板、输入区可选模板、AI 可填充模板占位符。
3. 对标豆包补体验增强（任务完成回写会话、图库筛选、重画、桌面版视觉输入）。

## 当前状态

待接手。前端测试基线 99/99 通过（2026-09-03），工作区 clean，主分支 main，版本 1.0.88。

## 背景与现状事实（已核实，接手前不必重查）

- 项目：Tauri 2 + Vue 3 双平台（桌面 `src-tauri/`，Web `src/api/` 适配器层），架构见 `docs/technical-design.md`，约束见 `docs/golden-principles.md`。
- Agent 工具循环：桌面 `src-tauri/src/services/agent.rs`（`run_turn`，最多 8 轮，envelope 自动修复降级）；Web `src/api/agent.js`。工具只有 `create_image_tasks`、`get_task_status`（`src-tauri/src/services/agent_tools.rs`）。
- 直接绘画：`src/components/AgentComposer.vue` 的「直接绘画」勾选 → `src/App.vue` `sendAgentConversationMessage` → `create_agent_direct_image_task`（`src-tauri/src/commands.rs:547`），已可用，本计划不动它。
- 图片库：`src/components/AgentLibraryPanel.vue`，已可用，本计划只加筛选和重画。
- 会话持久化：`src-tauri/src/services/agent_store.rs`（SQLite `agent_sessions` 表）；历史预算 48k/16k 字符、最近 24 条。

### 已确认的缺陷

1. **孤立 tool 消息（P0 根因）**：桌面端一轮工具调用结束后，`send_agent_message`（`commands.rs:131`）只落盘 `role=tool`（含 tool_call）的消息和最终 assistant 消息，**带 `tool_calls` 的中间 assistant 消息不落盘**；`task_group` 消息（`commands.rs:724`，`role=tool`、`tool_call: None`）也没有 tool_call_id。下一轮 `agent_message_to_chat_value`（`commands.rs:384`）原样转换，产出前面没有 assistant(tool_calls) 的 tool 消息 → 严格 OpenAI 兼容端点 400。宽松网关能容忍，所以平时不易发现。
2. **Web 端方向相反的配对断裂**：`finalizeSession`（`src/api/agent.js:614`）把 toolCall 挂在最终 assistant 消息上且只保留第一个，重建时 `agentMessageToChat`（`agent.js:564`）产出带 `tool_calls` 的 assistant 但后面没有 tool result 消息。
3. **`fill_prompt_template` 死代码**：命令（`commands.rs:1524`）、`template-fill` 流式事件、填充提示词（`chat.rs:16`）都在，前端与 adapter 零调用。
4. **模板按钮交互割裂**：聊天输入区「模板」按钮只是 `openDesign()`（`src/App.vue:741`），没有插入输入框的动作；Agent 无法访问模板。
5. **桌面版不给视觉模型传图**：桌面只传附件元数据（`commands.rs:407`），Web 传 `image_url`（`agent.js:548`），两端不一致。
6. 小问题：任务组画完后 LLM 不知道结果（除非主动调 `get_task_status`）；图库无来源筛选；无「再来一张」。

## 关键决策（接手时不要再问用户，按此执行）

1. **P0 修法选「重建时折叠」，不改会话存储结构、不做数据迁移。** 理由：存量会话里中间 assistant(tool_calls) 消息本来就缺失，改落盘结构救不了旧数据；在 `agent_message_to_chat_value`（桌面）和 `agentMessageToChat`（Web）重建时把历史 tool/task_group 消息折叠为 assistant 可读文本，新旧会话都覆盖，改动面最小。本轮工具调用期间的消息配对由 `run_turn` 在内存里自建，不受影响。
2. **模板接入走「Agent 工具 + plan 可选 templateId」**，符合项目「模型决策、执行端校验」原则；同时在输入区做轻量模板选择器（用户显式调用模板的路径）。
3. **`templateId` 与 `referencePolicy` 组合语义**：模板参考图追加进任务的参考图路径集合（去重）；`referencePolicy=none` 时忽略模板参考图，`use` 时仍要求 referenceIds 非空（模板图不替代用户显式附图）。
4. **AI 填充占位符复用现成命令 `fill_prompt_template`**，只补前端与 adapter；填充结果回填输入框，不直接入队，让用户保留最终修改权。
5. **任务完成回写**：`update_agent_task_group_summary` 把任务组标记 completed 时，追加一条轻量 `role=tool, status=task_result` 消息（含 taskGroupId 与输出路径元数据）；P0 的折叠规则必须覆盖这个新 status。
6. **桌面视觉输入默认关闭**：provider 配置新增可选 `chatVision` 布尔（归一化默认 false），UI 在 `ApiSourcePanel` 加复选框；开启后桌面按 Web 的 `image_url` 多部件格式传附件。
7. **不跑真实生图/对话冒烟**（需要真实 API Key，夜间无人值守不可靠）；所有验证用 mock 测试，真实冒烟留给用户人工验收。

## 待办

### P0 稳定性修复（先行，独立提交）

- [ ] **P0-1 桌面端历史重建折叠**：改 `agent_message_to_chat_value`（`commands.rs:384`）——历史消息中 `role=tool` 一律折叠为 assistant 文本：从 tool result JSON 里提取 `taskGroupId`、`tasks[].id/status`、`outputs[].path`、`error`（尽量解析，失败则原文截断 400 字符）；`status=task_group` 消息折叠为「已创建 N 个绘图任务（taskGroupId=…，状态 …）」。保证输出数组中不出现 `role=tool`。
- [ ] **P0-2 Web 端重建配对**：改 `agentMessageToChat`（`agent.js:564`）——assistant 带 `toolCall` 时，在其后注入配对的 `{role:"tool", tool_call_id, content}`（用 toolCall.result/error 构造，content 截断 400 字符）；`taskGroup`-only 消息折叠为 assistant 文本；`finalizeSession` 落盘全部 toolCalls（不再只留第一个）。
- [ ] **P0-3 回归测试**：桌面在 `src-tauri/tests/agent_integration.rs` 加「两轮对话」用例（mock provider，第一轮含工具调用，断言第二轮请求体无 `role=tool` 历史消息且含折叠摘要）；Web 在 `tests/api/agent.spec.js` 加对应用例。
- [ ] P0 完成：`pnpm verify` → 提交（`fix: 修复 Agent 多轮对话历史重建产生孤立 tool 消息`，Web/桌面可拆两个提交）。

### P1 模板闭环（需求核心）

- [ ] **P1-1 `list_templates` 工具**（只读）：`agent_tools.rs` 新增 schema（无参数）；`commands.rs` `execute_agent_tool` 加分支，返回 `{ templates: [{ id, title, preview(内容截断 200 字符), referenceCount }] }`，上限 50 条；`agent.js` 同步注册工具定义与执行分支；envelope 降级协议天然兼容（type=tool_call）。测试：Rust 集成 + `tests/api/agent.spec.js`。
- [ ] **P1-2 plan 可选 `templateId`**：`agent_tools.rs` schema 与 `validate_tool_arguments` 增加可选 `templateId`（保持 `additionalProperties: false` 语义下放行）；`create_agent_image_tasks_in_data_dir`（`commands.rs:617`）校验模板存在、按关键决策 3 合并模板参考图（走 `references.rs` 哈希去重路径）；`agent_system_prompt`（`commands.rs:339`）注入模板清单（id + 标题 + 一句话说明）与使用指示。测试：合法 templateId / 不存在 / policy=none 忽略模板图。
- [ ] **P1-3 输入区模板选择器**：`AgentComposer.vue`「模板」按钮改为轻量 popover（标题 + 内容预览 + 参考图缩略），选中后 emit 携带 templateId；`App.vue` 把模板内容填入输入框、模板参考图走现有附件上传路径挂载；不再 `openDesign()`。更新 `tests/components/AgentComposer.spec.js`、`AgentWorkspace.spec.js`。
- [ ] **P1-4 接通 AI 填充**：`adapter-tauri.js` 新增 `fillPromptTemplate(sessionId, providerId, content, onEvent)`（invoke + 监听 `template-fill` 事件）；`adapter-web.js` 用 chat provider fetch 实现同语义；`index.js` 导出同步（`adapter-consistency.spec.js` 会自动校验）。模板选择器里内容含 `{占位符}` 时显示「AI 填充」按钮，流式回填输入框。测试：adapter 层 mock 用例。
- [ ] P1 完成：`pnpm verify` → `pnpm ship 1.0.89` → 按任务拆分提交（`feat: Agent 新增 list_templates 工具` 等，Conventional Commits + 中文描述）。

### P2 体验增强（按序独立提交，时间不足可留到下次）

- [ ] **P2-1 任务完成回写会话**：按关键决策 5 追加 `task_result` 消息，并确保 P0 折叠规则覆盖新 status；不改 UI（任务组卡片已原地更新）。
- [ ] **P2-2 图库按来源筛选**：`src/lib/libraryFormat.js` 扩展来源分组/筛选（agent / 直接绘画 / 全部），`AgentLibraryPanel.vue` 顶部加筛选控件。
- [ ] **P2-3 重画/变体**：任务组卡片与图库 hover 操作加「再来一张」，用原任务 plan 重新入队（复用 `create_agent_image_tasks` / 直接任务命令，前端拼 plan）。
- [ ] **P2-4 桌面视觉输入**：按关键决策 6 实现 `chatVision` 设置与 `image_url` 传递，两端行为对齐。
- [ ] P2 完成：`pnpm verify` → `pnpm ship 1.0.90` → 独立提交。

## 执行策略（goal 模式）

- 每完成一个任务立即跑对应测试并 commit，禁止跨 Phase 混合提交；任何时刻中断，已提交内容即为有效进度。
- 阶段顺序严格为 P0 → P1 → P2；P0 未通过验证不得开始 P1。
- 每个 Phase 结束跑全量 `pnpm verify`（lint + fmt + test + cargo check/test），红字必须修复后再提交。
- **失败与重试**：执行器自身的模型请求遇到超时 / 429 / 5xx / 网络错误时指数退避自动重试（最多 5 次）；持续失败超过 30 分钟则 commit 当前进度、在计划文件「当前状态」标记断点后暂停。测试与验证全部 mock，不依赖真实模型可用性。
- **不询问用户**：所有方案分歧按「关键决策」执行；遇到决策未覆盖的新情况，选择与现有架构最一致、改动面最小的方案并在计划文件里补记。

## 硬约束（不可违反）

- 双端同步：新增 adapter 函数必须同时改 `adapter-tauri.js`、`adapter-web.js`、`index.js` 导出；新增 Agent 工具必须同步 schema、envelope 降级、两端集成测试（golden-principles 第 6 条）。
- UI 组件不持有业务状态，统一 `api.*` 调用，动作交回 `App.vue`。
- 删除文件走系统回收站；除 `~/.image-forge`、Tauri `app_data_dir()`、本仓库目录外删除需用户确认（无人值守期间一律不删计划外文件）。
- 交付说明必须报告实际移入回收站/删除的内容，没有也要明确说明。

## 未解决问题

- 折叠为 assistant 文本后，少数把 system 与 assistant 严格区分计费的网关计费口径会略变（低风险，可接受）。
- 模板内容本身很长时注入 system prompt 会挤占预算；P1 先做 200 字符 preview + 总量截断，后续再评估按需 `get_template` 工具。
- 桌面视觉输入的模型能力探测暂用手动开关，不做自动探测。

## 验收口径

1. 两轮含画图的 Agent 对话，第二轮请求体不出现孤立 tool 消息（桌面 + Web 各有测试证明）。
2. 聊天中：让 Agent「用某某模板画一张」→ Agent 调 `list_templates` → 提交带 `templateId` 的计划 → 入队成功；模板参考图正确进入任务。
3. 输入区选模板 → 内容与参考图进输入框/附件 → 可选 AI 填充 → 直接绘画或对话生成都可用。
4. `pnpm verify` 全绿，两个版本号 ship 完成（视 P2 完成度）。
