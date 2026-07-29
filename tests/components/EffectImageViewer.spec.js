import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import EffectImageViewer from "../../src/components/dialogs/EffectImageViewer.vue";

describe("EffectImageViewer", () => {
  it("在多张图片之间循环切换", async () => {
    const wrapper = mount(EffectImageViewer, {
      props: {
        show: true,
        items: [
          { path: "/tmp/one.png", title: "第一张" },
          { path: "/tmp/two.png", title: "第二张" },
        ],
      },
      global: {
        stubs: {
          "n-modal": { props: ["show"], template: "<div><slot /></div>" },
        },
      },
    });
    expect(wrapper.get("img").attributes("alt")).toBe("第一张");
    await wrapper.get("button[aria-label='下一张']").trigger("click");
    expect(wrapper.get("img").attributes("alt")).toBe("第二张");
    await wrapper.get("button[aria-label='上一张']").trigger("click");
    expect(wrapper.get("img").attributes("alt")).toBe("第一张");
  });
});
