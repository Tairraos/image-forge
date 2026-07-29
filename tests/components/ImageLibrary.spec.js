import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import ImageLibrary from "../../src/components/ImageLibrary.vue";

const tasks = [
  {
    id: "drawing-1",
    status: "completed",
    prompt: "山间日出",
    model: "gpt-image-2",
    completedAt: "2026-07-28T08:00:00+08:00",
    outputs: [{ path: "/tmp/sunrise.png", fileName: "sunrise.png", size: "1024x1024" }],
  },
  {
    id: "agent-1",
    status: "completed",
    prompt: "海边灯塔",
    model: "gpt-image-2",
    origin: "agent",
    agentSessionId: "session-1",
    completedAt: "2026-07-29T09:00:00+08:00",
    outputs: [
      { path: "/tmp/lighthouse-1.png", fileName: "lighthouse-1.png", size: "1024x1024" },
      { path: "/tmp/lighthouse-2.png", fileName: "lighthouse-2.png", size: "1024x1024" },
    ],
  },
];

describe("ImageLibrary", () => {
  it("按日期和批次显示完成图片", () => {
    const wrapper = mount(ImageLibrary, { props: { tasks } });
    expect(wrapper.findAll(".library-image-card")).toHaveLength(3);
    expect(wrapper.findAll(".image-day-group")).toHaveLength(2);
    expect(wrapper.text()).toContain("山间日出");
    expect(wrapper.text()).toContain("海边灯塔");
  });

  it("筛选 Agent 来源并打开当前图片集", async () => {
    const wrapper = mount(ImageLibrary, { props: { tasks } });
    const agentButton = wrapper.findAll(".image-source-filter button").find((button) => button.text() === "Agent");
    await agentButton.trigger("click");
    expect(wrapper.findAll(".library-image-card")).toHaveLength(2);
    await wrapper.find(".library-image-preview").trigger("click");
    const payload = wrapper.emitted("preview-images")[0][0];
    expect(payload.items).toHaveLength(2);
    expect(payload.index).toBe(0);
  });

  it("月份输入被清空后仍能显示完整日历", async () => {
    const wrapper = mount(ImageLibrary, { props: { tasks } });
    await wrapper.get('input[type="month"]').setValue("");
    expect(wrapper.findAll(".image-calendar-grid button")).toHaveLength(42);
  });
});
