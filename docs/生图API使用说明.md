# 生图 API 使用说明

本文档汇总 Image Forge 支持的图像生成服务的 API 使用说明：OpenAI（GPT Image）、xAI（Grok）、Google（Nano Banana / Gemini）。内容基于各厂商官方文档整理，供本地开发与联调参考。

> 文档链接：
> - OpenAI：[https://developers.openai.com/api/docs/guides/image-generation](https://developers.openai.com/api/docs/guides/image-generation)
> - Grok：[https://docs.x.ai/developers/model-capabilities/images/generation](https://docs.x.ai/developers/model-capabilities/images/generation)
> - Nano Banana：[https://ai.google.dev/gemini-api/docs/image-generation](https://ai.google.dev/gemini-api/docs/image-generation)

---

## 一、OpenAI（GPT Image 系列）

### 1.1 模型列表

| 模型 ID | 说明 |
| --- | --- |
| `gpt-image-2` | 最新旗舰模型，支持任意分辨率（含 4K）、图生图编辑。快照：`gpt-image-2-2026-04-21` |
| `gpt-image-1.5` | 侧重写实、指令跟随与多模态上下文 |
| `gpt-image-1` | 通用 GPT Image 模型 |
| `gpt-image-1-mini` | 轻量模型，适合快速原型与批量低门槛场景 |
| `dall-e-3` | 旧模型，标准尺寸生成 |

使用 GPT Image 系列模型前，通常需要在开发者控制台完成 **API Organization Verification（组织验证）**。

### 1.2 两种调用方式

**Image API**（`POST /v1/images/generations` 生成、`POST /v1/images/edits` 编辑）：适合单张图片的生成或编辑。

```bash
curl -X POST "https://api.openai.com/v1/images/generations" \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-image-2",
    "prompt": "A children book drawing of a veterinarian using a stethoscope",
    "n": 1,
    "size": "1024x1024",
    "quality": "high",
    "output_format": "png"
  }'
```

**Responses API**（内置 `image_generation` 工具）：适合对话式、多轮编辑场景。支持通过 `previous_response_id` 或 `image_generation_call.id` 引用上下文中的图片，持续迭代修改；`action` 参数可控制 `auto`（模型决定）/ `generate`（强制新建）/ `edit`（强制编辑，需上下文已有图片）。

```python
from openai import OpenAI

client = OpenAI()
response = client.responses.create(
    model="gpt-5.6",
    input="Generate an image of a gray tabby cat hugging an otter",
    tools=[{"type": "image_generation"}],
)
image_data = [o.result for o in response.output if o.type == "image_generation_call"]
```

### 1.3 输出参数

| 参数 | 取值 | 说明 |
| --- | --- | --- |
| `n` | 1–10 | 一次请求生成的图片数量，默认 1 |
| `quality` | `low` / `medium` / `high` / `auto` | 图片质量（GPT Image 系列）；`hd` / `standard` 用于 DALL·E 3 |
| `size` | 见下 | 图片尺寸 |
| `background` | `transparent` / `opaque` / `auto` | 背景透明度，仅 GPT Image 系列支持；透明需配合 `png` 或 `webp` 输出 |
| `output_format` | `png` / `jpeg` / `webp` | 输出格式（仅 GPT Image 系列） |
| `output_compression` | 0–100 | 压缩级别，配合 `webp` / `jpeg` 使用，默认 100 |
| `moderation` | `low` / `auto` | 内容审核严格度 |
| `partial_images` | 0–3 | 流式返回中间过程图数量，默认 0（只收最终图） |
| `prompt` | 最多 32000 字符 | 提示词（GPT Image 系列） |

> GPT Image 系列 **始终返回 base64（`b64_json`）**，不返回 URL，`response_format` 参数对其无效。

### 1.4 尺寸规则（重点：gpt-image-2 有硬性限制）

标准尺寸：`1024x1024`、`1536x1024`、`1024x1536`；`auto` 表示由模型自动选择。

对于 `gpt-image-2` 及快照 `gpt-image-2-2026-04-21`，**支持任意 `WIDTHxHEIGHT` 自定义分辨率**，但必须同时满足以下四条规则：

| 规则 | 限制 | 说明 |
| --- | --- | --- |
| 1. 长边上限 | **最长边 ≤ 3840 px** | 即单边长不能超过 3840 |
| 2. 像素总量 | **总像素在 655,360 ～ 8,294,400 之间** | 下限 655,360（如 1024×640），上限 8,294,400（即 3840×2160） |
| 3. 边长对齐 | 宽、高都必须能被 **16 整除** | 例如 1080 无效，需调整为 1088 |
| 4. 宽高比 | 长边与短边之比 **≤ 3:1**（即比例在 1:3 ～ 3:1 之间） | |

**说明**：其中第 1、2 条（长边 ≤ 3840、最大像素 8,294,400）在官方指南正文中未直接写出，但在 [API 参考的 size 参数](https://developers.openai.com/api/reference/resources/images/methods/generate)中明确提到"请求尺寸必须满足模型当前的像素与边缘限制"，且经实际接口调用验证确实生效——超出后接口会返回参数错误。因此在使用 `gpt-image-2` 自定义尺寸时，务必在客户端先做校验，避免无效请求。

常见合法尺寸示例：

| 尺寸 | 结论 | 说明 |
| --- | --- | --- |
| `3840x2160` | ✅ 合法（实验性） | 4K，总像素恰好达到上限 |
| `2160x3840` | ✅ 合法（实验性） | 竖向 4K |
| `4096x2160` | ❌ 非法 | 长边超过 3840 |
| `3840x1200` | ❌ 非法 | 宽高比 3.2:1，超过 3:1 |
| `1920x1080` | ❌ 非法 | 1080 不是 16 的倍数 |
| `1024x640` | ✅ 合法 | 总像素恰好为下限 |
| `2048x2048` | ✅ 合法 | 2K 方形 |

> 注意：分辨率高于 `2560x1440` 的输出目前标注为 **experimental（实验性）**，可能产生不稳定的结果。

---

## 二、xAI（Grok Imagine）

### 2.1 模型

| 模型 ID | 说明 |
| --- | --- |
| `grok-imagine-image-quality` | 当前推荐模型，用于文生图与图生图 |
| `grok-imagine-image` | 文生图、图生图编辑、多图编辑、风格迁移 |
| `grok-imagine-image-pro` | ⚠️ 已于 2026-05-15 弃用，请迁移至 `grok-imagine-image-quality` |

### 2.2 端点与请求

基础地址：`https://api.x.ai/v1`，**兼容 OpenAI SDK**（将 `base_url` 指向 `https://api.x.ai/v1` 即可）。

- 文生图：`POST /images/generations`
- 图生图编辑：`POST /images/edits`

```bash
# 文生图
curl -X POST https://api.x.ai/v1/images/generations \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $XAI_API_KEY" \
  -d '{
    "model": "grok-imagine-image",
    "prompt": "A collage of London landmarks in a stenciled street-art style"
  }'

# 图生图编辑
curl -X POST https://api.x.ai/v1/images/edits \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $XAI_API_KEY" \
  -d '{
    "model": "grok-imagine-image",
    "prompt": "Render this as a pencil sketch with detailed shading",
    "image": {
      "url": "https://docs.x.ai/assets/api-examples/images/style-realistic.png",
      "type": "image_url"
    }
  }'
```

### 2.3 关键能力

- **编辑输入图**：支持公开 URL 或 base64 data URI（`data:image/jpeg;base64,...`）两种形式。
- **多图编辑**：一次请求最多传 **5 张图**（`image_urls` 数组），输出宽高比默认跟随第一张输入图，可用 `aspect_ratio` 覆盖。
- **批量同提示**：`n` 参数（如 `n=4`）一次生成多张变体（对应 xAI SDK 的 `sample_batch()`）。
- **并发不同提示**：使用 `AsyncClient` + `asyncio.gather` 并发发送不同提示。
- **多轮编辑**：把上一轮输出作为下一轮输入，链式迭代（初始生成 → 改家具 → 调灯光）。
- **风格迁移**：在 prompt 中描述目标风格（油画、素描、波普、动漫、水彩等），对已有图片生效。
- **返回**：默认返回**临时 URL**（需及时下载），也可请求 base64 输出。

### 2.4 尺寸控制

通过 `aspect_ratio` 参数控制宽高比（如 `"1:1"`、`"16:9"`、`"3:2"` 等）；不传时由模型自动选择最优比例。

> 注意：OpenAI SDK 的 `images.edit()` 方法（multipart/form-data）**不适用**于 Grok，因为 xAI API 要求 `application/json`。请使用 xAI SDK、Vercel AI SDK 或直接 HTTP 请求。

---

## 三、Google（Nano Banana / Gemini）

### 3.1 模型列表

Nano Banana 是 Gemini 原生图片生成能力的名称，包含以下模型：

| 模型 ID | 名称 | 说明 |
| --- | --- | --- |
| `gemini-3.1-flash-lite-image` | Nano Banana 2 Lite | 最快、最便宜；仅支持 1K 图片 |
| `gemini-3.1-flash-image` | Nano Banana 2 | 通用主力模型，速度与 4K 生成兼顾；支持 0.5K~4K |
| `gemini-3-pro-image` | Nano Banana Pro | 专业级，最强推理、本地化与品牌一致性（曾用 `gemini-3-pro-image-preview`） |
| `gemini-2.5-flash-image` | Nano Banana（旧版） | 旧版先锋，官方建议迁移到 Nano Banana 2 Lite |

生成的所有图片都带 **SynthID 水印**。

### 3.2 端点与请求

- 新版推荐：**Interactions API**（`POST https://generativelanguage.googleapis.com/v1beta/interactions`）
- 传统方式：`POST /v1beta/models/{model}:generateContent`

```bash
curl -s -X POST \
  "https://generativelanguage.googleapis.com/v1beta/interactions" \
  -H "x-goog-api-key: $GEMINI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gemini-3.1-flash-image",
    "input": [
      {"type": "text", "text": "Create a picture of a nano banana dish in a fancy restaurant with a Gemini theme"}
    ]
  }'
```

### 3.3 输入与能力

- **文生图**：纯文本提示生成。
- **图生图编辑**：输入文本 + 图片（base64），支持添加/移除/修改元素、改样式、调色。
- **多轮修改**：通过 `previous_interaction_id` 或 `interactions.create` 连续对话迭代。
- **最多 14 张参考图**：Gemini 3 系列支持混合最多 14 张参考图（含角色一致性图片）。
- **Google 搜索接地（Grounding）**：`tools: [{"type": "google_search"}]`，基于实时数据（天气、股票、新闻）生成图片；3.1 Flash 还支持图片搜索。
- **思考过程（Thinking）**：Pro 模型利用"思考"推理复杂提示，先生成构思图再产出最终图（构思图不收费）。
- **视频转图片**（3.1 Flash）：可从视频帧生成图片。

### 3.4 尺寸与宽高比

- 通过 `image_config` 下的 `aspect_ratio` / `image_size` 字段控制输出。
- **分辨率档位**：`0.5K`（512px，仅 3.1 Flash）、`1K`（1024px）、`2K`（2048px）、`4K`（4096px）。**必须用大写 K**。
- **宽高比**：默认 1:1（或跟随输入图）；Pro 支持 `1:1`、`2:3`、`3:2`、`3:4`、`4:3`、`4:5`、`5:4`、`9:16`、`16:9`、`21:9`；3.1 Flash 另支持 `1:4`、`4:1`、`1:8`、`8:1` 等极端比例。
- **4K 上限**：最大 4096×4096（1:1 时）。

### 3.5 其他

- 最佳性能语言含中文（zh-CN）。
- 不支持音频、视频输入（视频转图片除外）。
- 输出为 base64（`interaction.output_image.data` 或响应中的 `inline_data`）。

---

## 四、三家模型对比速查

| 维度 | OpenAI gpt-image-2 | Grok grok-imagine-image | Nano Banana（Gemini） |
| --- | --- | --- | --- |
| 端点 | `/v1/images/*`、Responses API | `/v1/images/*`（兼容 OpenAI SDK） | `/v1beta/interactions`、`:generateContent` |
| 输出形式 | base64（无 URL） | 临时 URL 或 base64 | base64 |
| 最大分辨率 | 3840×2160（4K，实验性） | 由宽高比参数控制 | 4096×4096（4K） |
| 尺寸约束 | 长边 ≤3840、像素 655,360~8,294,400、边为 16 的倍数、比例 ≤3:1 | 通过 aspect_ratio 指定 | 0.5K/1K/2K/4K + 14 种宽高比 |
| 参考图数量 | 编辑最多 16 张（<50MB） | 最多 5 张 | 最多 14 张 |
| 多轮编辑 | Responses API 支持 | 链式调用支持 | 支持（多轮对话） |
| 特色 | 4K、透明背景（gpt-image-1.5） | 风格迁移、批量变体 | Google 搜索接地、思考模式、SynthID |
