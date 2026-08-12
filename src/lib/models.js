export function defaultSettings() {
  const provider = defaultProvider();
  return {
    activeProviderId: provider.id,
    activeImageProviderId: provider.id,
    activeChatProviderId: "",
    providers: [provider],
    outputDir: null,
    inputDir: null,
    autoStartQueue: true,
    autoRetry: false,
    notificationsEnabled: true,
  };
}

export const IMAGE_MODEL_TYPES = [
  "image-gpt",
  "image-agnes",
  "image-gemini",
  "image-grok",
  "image-seedream",
];

export function defaultProvider(index = 1, modelType = "image-gpt") {
  return {
    id: createProviderId(),
    name: index === 1 ? "默认" : `供应商 ${index}`,
    modelType,
    baseUrl: "https://api.openai.com/v1",
    apiKey: "",
    proxyUrl: "",
    imageModel: "gpt-image-2",
    imagesConcurrency: 1,
    enabled: true,
    notes: "",
  };
}

export function normalizeSettingsForUi(value) {
  const next = { ...defaultSettings(), ...value };
  next.providers = Array.isArray(next.providers) && next.providers.length ? next.providers : [defaultProvider()];
  next.providers = next.providers.map((provider, index) => ({
    ...defaultProvider(index + 1, "image-gpt"),
    ...provider,
    id: provider.id || createProviderId(),
    modelType: normalizeModelType(provider.modelType, provider.imageModel, provider.baseUrl),
    proxyUrl: provider.proxyUrl || "",
    imagesConcurrency: normalizeProviderConcurrency(provider.imagesConcurrency),
    notes: "",
  }));

  const imageProviders = next.providers.filter((provider) => isImageModelType(provider.modelType));
  const chatProviders = next.providers.filter((provider) => provider.modelType === "chat");
  const legacyActive = next.activeProviderId;

  next.activeImageProviderId = pickActiveProviderId(
    next.activeImageProviderId || legacyActive,
    imageProviders,
  );
  next.activeChatProviderId = pickActiveProviderId(next.activeChatProviderId, chatProviders);
  next.activeProviderId = next.activeImageProviderId || legacyActive || next.providers[0]?.id || "";
  return next;
}

export function emptyTemplate() {
  return {
    id: "",
    title: "",
    shortTitle: "",
    category: "常用",
    content: "",
    referencePaths: [],
    effectImagePath: "",
    notes: "",
    tags: [],
    favorite: false,
    usageCount: 0,
    modelHint: "",
    createdAt: "",
    updatedAt: "",
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

export function normalizeModelType(value, model = "", baseUrl = "") {
  if (value === "chat") return "chat";
  if (IMAGE_MODEL_TYPES.includes(value)) return value;
  return recommendImageModelType(model, baseUrl);
}

export function recommendImageModelType(model = "", baseUrl = "") {
  const hint = `${model} ${baseUrl}`.toLowerCase();
  if (/agnes/.test(hint)) return "image-agnes";
  if (/gemini|imagen|nano[ -]?banana/.test(hint)) return "image-gemini";
  if (/grok|api\.x\.ai/.test(hint)) return "image-grok";
  if (/seedream|doubao.*image|byteplus|volces|ark\./.test(hint)) return "image-seedream";
  return "image-gpt";
}

export function isImageModelType(value) {
  return value !== "chat";
}

export function normalizeProviderConcurrency(value) {
  void value;
  return 1;
}

function pickActiveProviderId(candidate, providers) {
  if (candidate && providers.some((provider) => provider.id === candidate)) {
    return candidate;
  }
  return providers[0]?.id || "";
}

export function parseClipboardProvider(raw) {
  const text = String(raw || "").trim();
  if (!text) return null;
  let data;
  try {
    data = JSON.parse(text);
  } catch {
    return null;
  }
  if (!data || typeof data !== "object" || Array.isArray(data)) return null;

  const nameMatch = text.match(/"name"\s*:\s*"([^"\\]*(?:\\.[^"\\]*)*)"/);
  const firstKey = Object.keys(data)[0] || "";
  let name = "";
  if (nameMatch?.[1]) {
    name = unescapeJsonString(nameMatch[1]).trim();
  } else {
    name = firstKey.trim();
  }

  const apiKey = firstProviderString(data, ["apiKey", "openAiApiKey"]);
  const baseUrl = firstProviderString(data, ["baseURL", "openAiBaseUrl", "baseUrl"]);
  if (!name || !apiKey || !baseUrl) return null;
  return { name, apiKey, baseUrl };
}

function firstProviderString(data, keys) {
  for (const key of keys) {
    const value = data?.[key];
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  if (data && typeof data === "object") {
    for (const value of Object.values(data)) {
      if (!value || typeof value !== "object" || Array.isArray(value)) continue;
      for (const key of keys) {
        const nested = value[key];
        if (typeof nested === "string" && nested.trim()) return nested.trim();
      }
    }
  }
  return "";
}

function unescapeJsonString(value) {
  try {
    return JSON.parse(`"${value}"`);
  } catch {
    return value;
  }
}

