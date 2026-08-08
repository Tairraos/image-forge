# 生图 API 使用说明

本文档汇总 Image Forge 支持的五家图像生成服务的 API 使用说明：OpenAI（GPT Image）、xAI（Grok）、Google（Nano Banana / Gemini）、火山引擎（即梦 Seedream）、Agnes AI（Sapiens AI）。内容基于各厂商官方文档整理，供本地开发与联调参考。

> 文档链接：
> - OpenAI：[https://developers.openai.com/api/docs/guides/image-generation](https://developers.openai.com/api/docs/guides/image-generation)
> - Grok：[https://docs.x.ai/developers/model-capabilities/images/generation](https://docs.x.ai/developers/model-capabilities/images/generation)
> - Nano Banana：[https://ai.google.dev/gemini-api/docs/image-generation](https://ai.google.dev/gemini-api/docs/image-generation)
> - Seedream：[https://docs.volcengine.com/docs/85621/2275082](https://docs.volcengine.com/docs/85621/2275082)
> - Agnes AI：[https://wiki.agnes-ai.com/zh-Hans/docs/agnes-image-21-flash](https://wiki.agnes-ai.com/zh-Hans/docs/agnes-image-21-flash)

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

## 四、火山引擎（即梦 Seedream 4.6）

### 4.1 接口简介

即梦图片 4.6 模型（`req_key: jimeng_seedream46_cvtob`），基于 Seedream 4.0 基础模型训练，聚焦修图垂类（人像写真、平面设计、图片风格化）。

采用**异步任务**模式：先提交任务拿到 `task_id`，再轮询或回调获取结果。

| 名称 | 内容 |
| --- | --- |
| 接口地址 | `https://visual.volcengineapi.com` |
| 请求方式 | POST，`Content-Type: application/json` |
| 鉴权 | 火山引擎签名（Region: `cn-north-1`，Service: `cv`，AK/SK） |

### 4.2 限制条件

| 名称 | 内容 |
| --- | --- |
| 输入图要求 | 仅支持 JPEG、PNG（建议 JPEG）；单图最大 **15MB**；最多 **14 张**；分辨率最大 **4096×4096** |
| 输出图说明 | 输出以列表返回；**最大输出图数量 = 15 − 输入图数量**，建议不超过 6 张 |
| 其他 | 分辨率越高、数量越多，延迟越大；按输出图片张数计费；对延迟/价格敏感可用 `force_single` 强制单图 |

### 4.3 提交任务

Query 参数：`?Action=CVSync2AsyncSubmitTask&Version=2022-08-31`

Body 参数：

| 参数 | 类型 | 必选 | 说明 |
| --- | --- | --- | --- |
| `req_key` | string | 是 | 固定值 `jimeng_seedream46_cvtob` |
| `image_urls` | array | 否 | 输入图 URL，0–14 张（建议 ≤ 6 张，过多会降低参考效果） |
| `prompt` | string | 是 | 提示词，中英文均可，**最长 800 字符**；不建议输入 `$` 等特殊符号 |
| `size` | int | 否 | 生成图片面积，默认 4194304（2048×2048，2K）；范围 [1024×1024, 4096×4096]；与宽高二选一，同时传时优先宽高 |
| `width` / `height` | int | 否 | 需同时传才生效；宽高乘积在 [1024×1024, 4096×4096]，宽高比在 [min_ratio, max_ratio] 内 |
| `scale` | int | 否 | 文本影响程度，默认 50，范围 [1, 100] |
| `force_single` | bool | 否 | 是否强制只生成单图，默认 false |
| `min_ratio` | float | 否 | 宽高比下限，默认 1/3，范围 [1/16, 16) |
| `max_ratio` | float | 否 | 宽高比上限，默认 3，范围 (1/16, 16] |
| `callback_url` | string | 否 | 异步回调 URL（需公网可访问） |
| `return_url` | bool | 否 | 回调时图片以链接返回（有效期 24 小时），默认 false |
| `logo_info` | JSON string | 否 | 明水印配置（`add_logo`、`position`、`language`、`opacity`、`logo_text_content`） |
| `aigc_meta` | JSON string | 否 | 隐式标识（`content_producer`、`producer_id` 等，依据《人工智能生成合成内容标识办法》） |

请求示例：

```json
{
  "req_key": "jimeng_seedream46_cvtob",
  "image_urls": ["https://xxxx"],
  "prompt": "背景换成白色",
  "width": 1024,
  "height": 1024,
  "force_single": false
}
```

返回：`code=10000` 表示成功，`data.task_id` 为任务 ID。

### 4.4 查询任务

Query 参数：`?Action=CVSync2AsyncGetResult&Version=2022-08-31`

Body 参数：`req_key`（同上）+ `task_id`（提交接口返回）+ 可选 `req_json`（JSON 字符串，可配置 `return_url`、`logo_info`、`aigc_meta`）。

返回字段：

| 字段 | 说明 |
| --- | --- |
| `binary_data_base64` | 图片 base64 数组 |
| `image_urls` | 图片 URL 数组（PNG 格式，有效期 24h） |
| `status` | `in_queue`（排队中）/ `generating`（处理中）/ `done`（完成）/ `not_found`（任务不存在或已过期 12h）/ `expired` |

> 解析时先判断外层 `code=10000`，再判断 `data.status`。

### 4.5 推荐输出尺寸

| 档位 | 尺寸 |
| --- | --- |
| 1K | `1024x1024`（1:1） |
| 2K | `2048x2048`（1:1）、`2304x1728`（4:3）、`2496x1664`（3:2）、`2560x1440`（16:9）、`3024x1296`（21:9） |
| 4K | `4096x4096`（1:1）、`4693x3520`（4:3）、`4992x3328`（3:2）、`5404x3040`（16:9）、`6197x2656`（21:9） |

### 4.6 常见业务错误码

| 错误码 | 描述 | 是否可重试 |
| --- | --- | --- |
| 50411 | 输入图片前审核未通过 | 否 |
| 50511 | 输出图片后审核未通过 | 可重试 |
| 50412 | 输入文本前审核未通过 | 否 |
| 50413 | 输入文本含敏感词、版权词 | 否 |
| 50518 | 输入版权图审核未通过 | 否 |
| 50519 | 输出版权图后审核未通过 | 可重试 |
| 50429 | QPS 超限 | 可重试 |
| 50430 | 并发超限 | 可重试 |
| 50500 | 内部错误 | 否 |

---

## 五、Agnes AI（Sapiens AI）

### 5.1 兼容性说明（重要）

Agnes AI 的 API 采用 **OpenAI 风格**（相同端点路径、Bearer 认证、响应结构），但**与 OpenAI 不完全兼容**，接入时需注意以下差异：

| 维度 | OpenAI | Agnes AI |
| --- | --- | --- |
| 端点 | `POST https://api.openai.com/v1/images/generations` | `POST https://apihub.agnes-ai.com/v1/images/generations` |
| 认证 | `Authorization: Bearer` | `Authorization: Bearer`（相同） |
| `response_format` | 放在请求体**顶层**（且对 GPT Image 系列无效） | 必须放在 **`extra_body` 内部**（`extra_body.response_format`），放顶层会报错 |
| 图生图输入 | `POST /images/edits`（multipart）或 Responses API | 在 `extra_body.image` 传入图片**数组**（URL 或 Data URI Base64），同一个 `/images/generations` 端点 |
| 图生图标记 | — | **不需要** `tags: ["img2img"]` |
| 批量数量 | 支持 `n`（1–10） | 单请求生成单张，**不支持 `n`** |
| 其他 OpenAI 参数 | `quality`、`background`、`output_format` 等 | **均不支持** |
| 专属参数 | — | `return_base64`（2.0 模型文生图返回 Base64 时使用） |

### 5.2 模型列表

| 模型 ID | 说明 |
| --- | --- |
| `agnes-image-2.1-flash` | 升级版，优化高信息密度图像、复杂构图与细节；支持文生图、图生图 |
| `agnes-image-2.0-flash` | 支持文生图、图生图、多图合成（多图输入、角色合成）；Artificial Analysis 图像编辑 ELO 1184，Top 20 |

### 5.3 端点与请求参数

**端点**：`POST https://apihub.agnes-ai.com/v1/images/generations`

**请求头**：`Authorization: Bearer YOUR_API_KEY` + `Content-Type: application/json`

**请求参数**：

| 参数 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `model` | string | 是 | 模型名称，`agnes-image-2.0-flash` 或 `agnes-image-2.1-flash` |
| `prompt` | string | 是 | 图像生成或编辑的文本指令 |
| `size` | string | 是 | 输出图像尺寸，如 `1024x768`、`1024x1024`、`768x1024`（自定义输出尺寸） |
| `image` | string[] | 图生图必填 | 输入图像数组，位于 `extra_body` 内；支持公网 URL 或 Data URI Base64（`data:image/png;base64,...`） |
| `return_base64` | boolean | 否 | 文生图需要以 Base64 返回时使用（2.0 模型） |
| `extra_body.response_format` | string | 否 | 输出格式，枚举：`url`、`b64_json` |

### 5.4 请求示例

**文生图（URL 输出）**：

```bash
curl https://apihub.agnes-ai.com/v1/images/generations \
 -H "Authorization: Bearer YOUR_API_KEY" \
 -H "Content-Type: application/json" \
 -d '{
 "model": "agnes-image-2.1-flash",
 "prompt": "A luminous floating city above a misty canyon at sunrise, cinematic realism",
 "size": "1024x768",
 "extra_body": {
   "response_format": "url"
 }
 }'
```

返回路径：`data[0].url`

**图生图（Base64 输出）**：

```bash
curl https://apihub.agnes-ai.com/v1/images/generations \
 -H "Authorization: Bearer YOUR_API_KEY" \
 -H "Content-Type: application/json" \
 -d '{
 "model": "agnes-image-2.1-flash",
 "prompt": "Make the object orange while preserving the original composition",
 "size": "1024x768",
 "extra_body": {
   "image": ["https://example.com/input.png"],
   "response_format": "b64_json"
 }
 }'
```

返回路径：`data[0].b64_json`

### 5.5 响应格式

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `created` | integer | 请求创建时间戳 |
| `data` | array | 生成的图像结果列表 |
| `data[].url` | string / null | 生成图像 URL，Base64 输出时通常为 `null` |
| `data[].b64_json` | string / null | Base64 图像数据，URL 输出时通常为 `null` |
| `data[].revised_prompt` | string / null | 修正后的提示词；没有时为 `null` |

### 5.6 常见错误与注意事项

| 问题 | 处理方式 |
| --- | --- |
| `response_format` 放顶层报错 | 放入 `extra_body` 内部，如 `extra_body.response_format: "url"` |
| 图生图传 `tags: ["img2img"]` | 不需要，直接在 `extra_body.image` 提供输入图即可 |
| 图生图缺少 `image` | 图生图时 `extra_body.image` 为必填项 |
| 输入图 URL 无法访问 | 使用公网可访问的 HTTPS 图片 URL；无法公开访问时改用 Data URI Base64 |
| 请求超时 | 生成可能需要数秒到数十秒，客户端超时建议 `60s - 360s` |

**定价**：标准 `$0.003 / 张`，当前 `$0 / 张`。

### 5.7 接入检查清单

- [ ] 请求 URL 为 `https://apihub.agnes-ai.com/v1/images/generations`
- [ ] 模型名称为 `agnes-image-2.0-flash` / `agnes-image-2.1-flash`
- [ ] 文生图请求不传 `image`，仅需 `model`、`prompt`、`size`
- [ ] 图生图请求在 `extra_body.image` 中传入图片数组
- [ ] `response_format` 放在 `extra_body` 内部，不传 `tags: ["img2img"]`

---

## 六、五家模型对比速查

| 维度 | OpenAI gpt-image-2 | Grok grok-imagine-image | Nano Banana（Gemini） | 即梦 Seedream 4.6 | Agnes Image 2.1 Flash |
| --- | --- | --- | --- | --- | --- |
| 端点 | `/v1/images/*`、Responses API | `/v1/images/*`（兼容 OpenAI SDK） | `/v1beta/interactions`、`:generateContent` | `visual.volcengineapi.com`（异步任务） | `apihub.agnes-ai.com/v1/images/generations` |
| 输出形式 | base64（无 URL） | 临时 URL 或 base64 | base64 | base64 或 URL（24h） | URL 或 base64 |
| 最大分辨率 | 3840×2160（4K，实验性） | 由宽高比参数控制 | 4096×4096（4K） | 4096×4096（4K） | 自定义尺寸（如 1024x768），文档未公布上限 |
| 尺寸约束 | 长边 ≤3840、像素 655,360~8,294,400、边为 16 的倍数、比例 ≤3:1 | 通过 aspect_ratio 指定 | 0.5K/1K/2K/4K + 14 种宽高比 | 面积或宽高，比例 min/max 限制 | 任意 `WxH` 字符串，由调用方指定 |
| 参考图数量 | 编辑最多 16 张（<50MB） | 最多 5 张 | 最多 14 张 | 最多 14 张（15MB/张） | 图生图支持图片数组（未公布上限） |
| 多轮编辑 | Responses API 支持 | 链式调用支持 | 支持（多轮对话） | 不支持（单次任务） | 不支持（单次请求） |
| 特色 | 4K、透明背景（gpt-image-1.5） | 风格迁移、批量变体 | Google 搜索接地、思考模式、SynthID | 人像写真、修图垂类、水印/隐式标识 | 高信息密度、多图合成、当前免费 |
