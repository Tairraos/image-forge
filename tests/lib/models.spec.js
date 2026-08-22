import { describe, expect, it } from "vitest";
import {
  parseClipboardProvider,
  recommendImageModelType,
} from "../../src/lib/models";

describe("models provider helpers", () => {
  it("parses clipboard provider json", () => {
    const parsed = parseClipboardProvider(
      JSON.stringify({
        name: "Bazaar",
        apiKey: "sk-test",
        baseURL: "https://bazaarlink.ai/api/v1",
      }),
    );
    expect(parsed).toEqual({
      name: "Bazaar",
      apiKey: "sk-test",
      baseUrl: "https://bazaarlink.ai/api/v1",
    });
  });

  it("uses first key when name missing", () => {
    const parsed = parseClipboardProvider(
      JSON.stringify({
        MyGate: {
          openAiApiKey: "sk-nested",
          openAiBaseUrl: "https://example.com/v1",
        },
      }),
    );
    expect(parsed).toEqual({
      name: "MyGate",
      apiKey: "sk-nested",
      baseUrl: "https://example.com/v1",
    });
  });

  it("rejects incomplete clipboard json", () => {
    expect(parseClipboardProvider('{"name":"x"}')).toBeNull();
  });

  it("parses free-text clipboard with url key name and image model", () => {
    const parsed = parseClipboardProvider(
      [
        "我的绘图源",
        "https://api.example.com/v1",
        "sk-abc123def456",
        "gpt-image-2",
      ].join("\n"),
    );
    expect(parsed).toEqual({
      name: "我的绘图源",
      apiKey: "sk-abc123def456",
      baseUrl: "https://api.example.com/v1",
      imageModel: "gpt-image-2",
    });
  });

  it("parses quoted free-text clipboard tokens", () => {
    const parsed = parseClipboardProvider(
      'name: "OpenRouter"\nurl: "https://openrouter.ai/api/v1"\nkey: "sk-or-v1-xxxx"\nmodel: "gemini-2.5-flash-image"',
    );
    expect(parsed).toEqual({
      name: "OpenRouter",
      apiKey: "sk-or-v1-xxxx",
      baseUrl: "https://openrouter.ai/api/v1",
      imageModel: "gemini-2.5-flash-image",
    });
  });

  it("rejects free-text without both key and url", () => {
    expect(parseClipboardProvider("https://api.openai.com/v1\ngpt-image-2")).toBeNull();
    expect(parseClipboardProvider("sk-only-key")).toBeNull();
    expect(parseClipboardProvider("not-json")).toBeNull();
  });

  it("recommends model type from pasted image model name", () => {
    expect(recommendImageModelType("grok-imagine-image-quality", "")).toBe("image-grok");
    expect(recommendImageModelType("gemini-2.5-flash-image", "")).toBe("image-gemini");
  });
});
