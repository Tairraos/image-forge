import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import AppFooterBar from "../../src/components/AppFooterBar.vue";

const PopselectStub = {
  props: ["value", "options", "placement", "trigger"],
  emits: ["update:value"],
  template: '<div class="popselect-stub"><slot /></div>',
};

describe("AppFooterBar", () => {
  it("显示生图和对话 API，并让两个菜单向上展开", async () => {
    const wrapper = mount(AppFooterBar, {
      props: {
        imageProviderId: "image-1",
        imageProviderName: "生图源",
        imageProviderOptions: [{ label: "生图源", value: "image-1" }],
        chatProviderId: "chat-1",
        chatProviderName: "对话源",
        chatProviderOptions: [{ label: "对话源", value: "chat-1" }],
      },
      global: { stubs: { NPopselect: PopselectStub } },
    });

    expect(wrapper.findAll(".status-api-name").map((item) => item.text())).toEqual(["生图源", "对话源"]);
    expect(wrapper.get(".status-api-separator").text()).toBe("/");
    const selectors = wrapper.findAllComponents(PopselectStub);
    expect(selectors).toHaveLength(2);
    expect(selectors.every((selector) => selector.props("placement") === "top-start")).toBe(true);
    selectors[0].vm.$emit("update:value", "image-2");
    selectors[1].vm.$emit("update:value", "chat-2");
    await wrapper.vm.$nextTick();
    expect(wrapper.emitted("select-image-provider")).toEqual([["image-2"]]);
    expect(wrapper.emitted("select-chat-provider")).toEqual([["chat-2"]]);
  });
});
