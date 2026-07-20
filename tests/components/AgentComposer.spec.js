import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import AgentComposer from "../../src/components/AgentComposer.vue";

describe("AgentComposer", () => {
  it("普通消息发送给对话模型", async () => {
    const wrapper = mount(AgentComposer, { props: { providerId: "chat", imageProviderId: "image" } });
    await wrapper.get("textarea").setValue("你好 Agent");
    await wrapper.get(".agent-send-button").trigger("click");
    expect(wrapper.emitted("send")).toEqual([[{ content: "你好 Agent", drawThisTurn: false }]]);
  });

  it("勾选本轮绘画后发送绘画请求，并支持停止与参考图删除", async () => {
    const wrapper = mount(AgentComposer, {
      props: {
        providerId: "chat",
        imageProviderId: "image",
        attachments: [{ id: "ref-1", dataUrl: "data:image/png;base64,AA==", fileName: "ref.png" }],
      },
    });
    await wrapper.get('input[type="checkbox"]').setValue(true);
    await wrapper.get("textarea").setValue("画一只猫");
    await wrapper.get(".agent-send-button").trigger("click");
    expect(wrapper.emitted("send")).toEqual([[{ content: "画一只猫", drawThisTurn: true }]]);
    await wrapper.get('button[aria-label="移除参考图"]').trigger("click");
    expect(wrapper.emitted("remove-attachment")).toEqual([["ref-1"]]);
    await wrapper.setProps({ busy: true });
    expect(wrapper.text()).toContain("停止");
  });
});
