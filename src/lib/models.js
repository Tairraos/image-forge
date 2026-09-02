export function defaultSettings() {
  const provider = defaultProvider();
  return {
    activeProviderId: provider.id,
    activeImageProviderId: provider.id,
    activeChatProviderId: '',
    providers: [provider],
    outputDir: null,
    inputDir: null,
    autoStartQueue: true,
    autoRetry: false,
    notificationsEnabled: true,
  };
}

export const IMAGE_MODEL_TYPES = ['image-gpt', 'image-gemini', 'image-grok'];

/** 绘图 API 模型类型下拉选项（供设置面板使用） */
export const IMAGE_MODEL_TYPE_OPTIONS = [
  { label: 'GPT / OpenAI 兼容', value: 'image-gpt' },
  { label: 'Gemini / Nano Banana', value: 'image-gemini' },
  { label: 'Grok / xAI', value: 'image-grok' },
];

/** 绘图 API 默认模型 ID */
export const DEFAULT_IMAGE_MODEL = 'gpt-image-2';
/** 对话 API 默认模型 ID */
export const DEFAULT_CHAT_MODEL = 'gpt-5.6-luna';

/**
 * 创建默认 API 源。
 * @param {number} index 序号（用于默认名称）
 * @param {string} modelType 模型类型：image-* 或 chat
 */
export function defaultProvider(index = 1, modelType = 'image-gpt') {
  const isChat = modelType === 'chat';
  return {
    id: createProviderId(),
    name: index === 1 ? '默认' : `供应商 ${index}`,
    modelType,
    baseUrl: 'https://api.openai.com/v1',
    apiKey: '',
    proxyUrl: '',
    // 对话默认 gpt-5.6-luna；绘图默认 gpt-image-2
    imageModel: isChat ? DEFAULT_CHAT_MODEL : DEFAULT_IMAGE_MODEL,
    imagesConcurrency: 1,
    chatVision: false,
    enabled: true,
    notes: '',
  };
}

export function normalizeSettingsForUi(value) {
  const next = { ...defaultSettings(), ...value };
  next.providers =
    Array.isArray(next.providers) && next.providers.length ? next.providers : [defaultProvider()];
  next.providers = next.providers.map((provider, index) => ({
    ...defaultProvider(index + 1, 'image-gpt'),
    ...provider,
    id: provider.id || createProviderId(),
    modelType: normalizeModelType(provider.modelType, provider.imageModel, provider.baseUrl),
    proxyUrl: provider.proxyUrl || '',
    imagesConcurrency: 1,
    chatVision: Boolean(provider.chatVision),
    notes: '',
  }));

  const imageProviders = next.providers.filter((provider) => isImageModelType(provider.modelType));
  const chatProviders = next.providers.filter((provider) => provider.modelType === 'chat');
  const legacyActive = next.activeProviderId;

  next.activeImageProviderId = pickActiveProviderId(
    next.activeImageProviderId || legacyActive,
    imageProviders
  );
  next.activeChatProviderId = pickActiveProviderId(next.activeChatProviderId, chatProviders);
  next.activeProviderId = next.activeImageProviderId || legacyActive || next.providers[0]?.id || '';
  return next;
}

export function emptyTemplate() {
  return {
    id: '',
    title: '',
    shortTitle: '',
    category: '常用',
    content: '',
    referencePaths: [],
    effectImagePath: '',
    notes: '',
    tags: [],
    favorite: false,
    usageCount: 0,
    modelHint: '',
    createdAt: '',
    updatedAt: '',
  };
}

export function deepClone(value) {
  return JSON.parse(JSON.stringify(value));
}

export function createProviderId() {
  if (globalThis.crypto?.randomUUID) {
    return `provider-${globalThis.crypto.randomUUID()}`;
  }
  return `provider-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export function normalizeModelType(value, model = '', baseUrl = '') {
  if (value === 'chat') return 'chat';
  if (IMAGE_MODEL_TYPES.includes(value)) return value;
  return recommendImageModelType(model, baseUrl);
}

export function recommendImageModelType(model = '', baseUrl = '') {
  const hint = `${model} ${baseUrl}`.toLowerCase();
  if (/gemini|imagen|nano[ -]?banana/.test(hint)) return 'image-gemini';
  if (/grok|api\.x\.ai/.test(hint)) return 'image-grok';
  return 'image-gpt';
}

export function isImageModelType(value) {
  return value !== 'chat';
}

function pickActiveProviderId(candidate, providers) {
  if (candidate && providers.some((provider) => provider.id === candidate)) {
    return candidate;
  }
  return providers[0]?.id || '';
}

/**
 * 从剪贴板文本解析 API 源配置。
 *
 * 调用入口：
 * - 组件：src/components/dialogs/ApiSourcePanel.vue
 * - 函数：pasteProvider()
 * - 触发：设置对话框 → 绘图 API / 对话 API 面板底部「粘贴」按钮
 *
 * 解析策略（按优先级）：
 * 1. JSON 对象：兼容常见字段名（name / apiKey / baseURL 等，含嵌套对象）
 * 2. 纯文本启发式：按行/引号串提取 baseUrl、apiKey、name、imageModel
 *
 * 成功时至少需要 apiKey + baseUrl；name / imageModel 可选。
 * 失败返回 null，由调用方触发按钮抖动反馈。
 *
 * @param {string} raw 剪贴板原始文本
 * @returns {{ name: string, apiKey: string, baseUrl: string, imageModel?: string } | null}
 */
export function parseClipboardProvider(raw) {
  const text = String(raw || '').trim();
  if (!text) return null;

  // —— 策略 1：优先尝试 JSON ——
  // 兼容直接粘贴 JSON 配置，或把供应商名作为顶层 key 的嵌套结构。
  const fromJson = parseClipboardProviderJson(text);
  if (fromJson) return fromJson;

  // —— 策略 2：纯文本启发式 ——
  // 支持多行备注、引号包围值、混杂说明文字的自由格式粘贴。
  return parseClipboardProviderText(text);
}

/**
 * JSON 解析路径。
 * 支持两种常见形状：
 * 1) { "name": "...", "apiKey": "...", "baseURL": "..." }
 * 2) { "供应商名": { "openAiApiKey": "...", "openAiBaseUrl": "..." } }
 */
function parseClipboardProviderJson(text) {
  let data;
  try {
    data = JSON.parse(text);
  } catch {
    return null;
  }
  if (!data || typeof data !== 'object' || Array.isArray(data)) return null;

  // 优先用源文本里 "name" 字段的字面量，避免 JSON 解析后键顺序变化；
  // 若没有 name 字段，则退回第一个顶层 key 作为名称（嵌套配置场景）。
  const nameMatch = text.match(/"name"\s*:\s*"([^"\\]*(?:\\.[^"\\]*)*)"/);
  const firstKey = Object.keys(data)[0] || '';
  let name;
  if (nameMatch?.[1]) {
    name = unescapeJsonString(nameMatch[1]).trim();
  } else {
    name = firstKey.trim();
  }

  const apiKey = firstProviderString(data, ['apiKey', 'openAiApiKey', 'api_key', 'key']);
  const baseUrl = firstProviderString(data, [
    'baseURL',
    'openAiBaseUrl',
    'baseUrl',
    'base_url',
    'url',
  ]);
  // 模型字段可选：有则带出，便于后续自动推测 modelType
  const imageModel = firstProviderString(data, [
    'imageModel',
    'openAiModelId',
    'model',
    'modelId',
    'model_id',
  ]);

  if (!apiKey || !baseUrl) return null;
  // name 缺失时允许空串，由调用方或文本启发式补全；此处至少保证可创建源
  return {
    name: name || '',
    apiKey,
    baseUrl,
    ...(imageModel ? { imageModel } : {}),
  };
}

/**
 * 纯文本启发式解析路径。
 *
 * 规则（用户约定）：
 * 1. baseUrl：http:// 或 https:// 开头的完整行，或被单/双引号包围的串
 * 2. apiKey：sk- 开头的整行或被引号包围的串
 * 3. name：第一行若以中/英文开头，且长度 ≤16 汉字或 ≤30 字母，在找不到更明确名字时用作名称
 * 4. imageModel：纯英文（可含 /）且包含 "image"、长度 ≤30 的一行/引号串；
 *    再结合 recommendImageModelType 推测模型类型（由调用方消费）
 *
 * 成功条件：至少解析出 apiKey + baseUrl。
 */
function parseClipboardProviderText(text) {
  // 拆成非空行，保留原始顺序（名称候选依赖“第一行”）
  const lines = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);

  // 同时扫描：整行 + 行内被 '...' 或 "..." 包围的片段
  const tokens = collectClipboardTokens(lines);

  let baseUrl = '';
  let apiKey = '';
  let imageModel = '';
  let name = '';

  // 1) Base URL：http(s):// 开头
  for (const token of tokens) {
    if (!baseUrl && isClipboardBaseUrl(token)) {
      baseUrl = token;
      break;
    }
  }

  // 2) API Key：sk- 开头（允许 sk-proj- 等变体）
  for (const token of tokens) {
    if (!apiKey && isClipboardApiKey(token)) {
      apiKey = token;
      break;
    }
  }

  // 3) 模型名：纯英文（可含 / - _ .），包含 image，长度 ≤30
  for (const token of tokens) {
    if (!imageModel && isClipboardImageModel(token)) {
      imageModel = token;
      break;
    }
  }

  // 4) 名称：优先第一行短标题；排除 url/key/model，以及 "key: value" 标签行
  const firstLine = lines[0] || '';
  if (isClipboardNameCandidate(firstLine) && !isClipboardLabeledLine(firstLine)) {
    name = firstLine;
  }

  // 若第一行不可用，再在其余 token 中找一个更干净的短名称候选
  // 优先选择“不像 label: value”的纯名称（例如引号内的 OpenRouter）
  if (!name) {
    for (const token of tokens) {
      if (
        token === baseUrl ||
        token === apiKey ||
        token === imageModel ||
        isClipboardBaseUrl(token) ||
        isClipboardApiKey(token) ||
        isClipboardImageModel(token) ||
        isClipboardLabeledLine(token)
      ) {
        continue;
      }
      if (isClipboardNameCandidate(token)) {
        name = token;
        break;
      }
    }
  }

  if (!apiKey || !baseUrl) return null;
  return {
    name: name || '',
    apiKey,
    baseUrl,
    ...(imageModel ? { imageModel } : {}),
  };
}

/**
 * 收集剪贴板中的候选 token：
 * - 每一整行
 * - 行内被单引号或双引号包围的子串
 * 去重但保序。
 */
function collectClipboardTokens(lines) {
  const tokens = [];
  const seen = new Set();
  const push = (value) => {
    const token = String(value || '').trim();
    if (!token || seen.has(token)) return;
    seen.add(token);
    tokens.push(token);
  };

  for (const line of lines) {
    push(line);
    // 匹配 "..." 或 '...'，不处理转义嵌套（自由文本场景足够）
    const quoted = line.matchAll(/(['"])(.*?)\1/g);
    for (const match of quoted) {
      push(match[2]);
    }
  }
  return tokens;
}

/** 是否为 http(s) Base URL */
function isClipboardBaseUrl(value) {
  return /^https?:\/\/\S+$/i.test(String(value || '').trim());
}

/** 是否为 sk- 开头的 API Key（整段无空白） */
function isClipboardApiKey(value) {
  return /^sk-[^\s'"]+$/i.test(String(value || '').trim());
}

/**
 * 是否像绘图模型 ID：
 * - 仅英文、数字、以及 / - _ .
 * - 忽略大小写包含 "image"
 * - 长度 ≤ 30
 */
function isClipboardImageModel(value) {
  const token = String(value || '').trim();
  if (!token || token.length > 30) return false;
  if (!/^[A-Za-z0-9/_.-]+$/.test(token)) return false;
  return /image/i.test(token);
}

/**
 * 是否可用作 API 源名称：
 * - 以中文或英文字母开头
 * - 中文（含扩展）字符数 ≤ 16，或纯字母数字等拉丁字符长度 ≤ 30
 * - 不允许整段是 URL / Key / 模型 ID
 */
function isClipboardNameCandidate(value) {
  const token = String(value || '').trim();
  if (!token) return false;
  if (isClipboardBaseUrl(token) || isClipboardApiKey(token) || isClipboardImageModel(token)) {
    return false;
  }
  // 必须以中文或英文开头
  if (!/^[\u4e00-\u9fffA-Za-z]/.test(token)) return false;

  // 统计汉字数量与整体长度
  const hanChars = token.match(/[\u4e00-\u9fff]/g) || [];
  if (hanChars.length > 0) {
    // 含中文：汉字不超过 16；整体也不宜过长
    return hanChars.length <= 16 && token.length <= 32;
  }
  // 纯拉丁：不超过 30 个字符
  return token.length <= 30;
}

/**
 * 是否为 "key: value" / "key=value" 形式的标签行。
 * 这类整行不适合直接当名称，应优先取其中的引号值。
 */
function isClipboardLabeledLine(value) {
  return /^[A-Za-z\u4e00-\u9fff_][\w\u4e00-\u9fff.-]*\s*[:=]\s*\S/.test(String(value || '').trim());
}

/**
 * 在 JSON 对象（含一层嵌套）中按候选 key 列表取第一个非空字符串。
 * 用于兼容 apiKey / openAiApiKey / baseURL / openAiBaseUrl 等别名。
 */
function firstProviderString(data, keys) {
  for (const key of keys) {
    const value = data?.[key];
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  if (data && typeof data === 'object') {
    for (const value of Object.values(data)) {
      if (!value || typeof value !== 'object' || Array.isArray(value)) continue;
      for (const key of keys) {
        const nested = value[key];
        if (typeof nested === 'string' && nested.trim()) return nested.trim();
      }
    }
  }
  return '';
}

/** 还原 JSON 字符串字面量中的转义字符 */
function unescapeJsonString(value) {
  try {
    return JSON.parse(`"${value}"`);
  } catch {
    return value;
  }
}
