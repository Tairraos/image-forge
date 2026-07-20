import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import AppTopbar from "../../src/components/AppTopbar.vue";

describe("AppTopbar 模式切换", () => {
  it("从绘画模式切换到 Agent，并仅在 Agent 模式展示 Skill", async () => {
    const wrapper = mount(AppTopbar, { props: { mode: "drawing" } });
    expect(wrapper.text()).not.toContain("Skill");
    await wrapper.get('button[title^="Agent 模式"]').trigger("click");
    expect(wrapper.emitted("update:mode")).toEqual([["agent"]]);
    await wrapper.setProps({ mode: "agent" });
    expect(wrapper.text()).toContain("Skill");
    expect(wrapper.get('button[title^="Agent 模式"]').attributes("aria-pressed")).toBe("true");
  });
});
