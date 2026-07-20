import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const root = resolve(import.meta.dirname, "../..");

describe("tauri bundle config", () => {
  it("disables macOS window restoration in packaged apps", () => {
    const config = JSON.parse(readFileSync(resolve(root, "src-tauri/tauri.conf.json"), "utf8"));
    const plist = readFileSync(resolve(root, "src-tauri/Info.plist"), "utf8");

    expect(config.bundle.macOS.infoPlist).toBe("Info.plist");
    expect(plist).toContain("<key>NSQuitAlwaysKeepsWindows</key>");
    expect(plist).toContain("<false/>");
  });
});
