import { describe, expect, it } from "vitest";
import { logicalWindowSize } from "../../src/tauri";

describe("窗口状态尺寸恢复", () => {
  it("把旧版物理像素状态转换为逻辑像素并应用最小尺寸", () => {
    expect(logicalWindowSize({ width: 1200, height: 800 }, 2, { width: 3000, height: 2000 }))
      .toEqual({ width: 1200, height: 800 });
  });

  it("新版逻辑像素状态不会被 Retina 缩放再次除以二", () => {
    expect(logicalWindowSize(
      { width: 1360, height: 930, unit: "logical" },
      2,
      { width: 3000, height: 2000 },
    )).toEqual({ width: 1360, height: 930 });
  });
});
