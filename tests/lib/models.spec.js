import { describe, expect, it } from "vitest";
import {
  normalizeProviderConcurrency,
  parseClipboardProvider,
  recommendImageModelType,
} from "../../src/lib/models";

describe("models provider helpers", () => {
  it("fixes concurrency to 1", () => {
    expect(normalizeProviderConcurrency(8)).toBe(1);
  });

  it("detects agnes model type", () => {
    expect(recommendImageModelType("agnes-image-2.1-flash")).toBe("image-agnes");
  });

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
    expect(parseClipboardProvider("not-json")).toBeNull();
  });
});
