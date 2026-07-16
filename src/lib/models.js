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
    imagesConcurrency: 1,
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
  if (/gemini|imagen|nano[ -]?banana/.test(hint)) return "image-gemini";
  if (/grok|api\.x\.ai/.test(hint)) return "image-grok";
  if (/seedream|doubao.*image|byteplus|volces|ark\./.test(hint)) return "image-seedream";
  return "image-gpt";
}

export function isImageModelType(value) {
  return value !== "chat";
}

function pickActiveProviderId(candidate, providers) {
  if (candidate && providers.some((provider) => provider.id === candidate)) {
    return candidate;
  }
  return providers[0]?.id || "";
}
