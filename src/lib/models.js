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

export function defaultProvider(index = 1, modelType = "image") {
  return {
    id: createProviderId(),
    name: index === 1 ? "Default" : `Provider ${index}`,
    modelType,
    baseUrl: "https://api.openai.com/v1",
    apiKey: "",
    imageModel: "gpt-image-2",
    imagesConcurrency: 4,
    enabled: true,
    notes: "",
  };
}

export function normalizeSettingsForUi(value) {
  const next = { ...defaultSettings(), ...value };
  next.providers = Array.isArray(next.providers) && next.providers.length ? next.providers : [defaultProvider()];
  next.providers = next.providers.map((provider, index) => ({
    ...defaultProvider(index + 1, "image"),
    ...provider,
    id: provider.id || createProviderId(),
    modelType: normalizeModelType(provider.modelType),
  }));

  const imageProviders = next.providers.filter((provider) => provider.modelType === "image");
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

export function normalizeModelType(value) {
  return value === "chat" ? "chat" : "image";
}

function pickActiveProviderId(candidate, providers) {
  if (candidate && providers.some((provider) => provider.id === candidate)) {
    return candidate;
  }
  return providers[0]?.id || "";
}
