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

  it("回车和 Command 回车发送，Shift 回车保留换行", async () => {
    const wrapper = mount(AgentComposer, { props: { providerId: "chat", imageProviderId: "image" } });
    const textarea = wrapper.get("textarea");
    await textarea.setValue("第一条");
    await textarea.trigger("keydown", { key: "Enter" });
    await textarea.setValue("需要换行");
    await textarea.trigger("keydown", { key: "Enter", shiftKey: true });
    await textarea.trigger("keydown", { key: "Enter", isComposing: true });
    await textarea.setValue("第二条");
    await textarea.trigger("keydown", { key: "Enter", metaKey: true });
    expect(wrapper.emitted("send")).toEqual([
      [{ content: "第一条", drawThisTurn: false }],
      [{ content: "第二条", drawThisTurn: false }],
    ]);
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
    expect(wrapper.text()).toContain("直接绘画");
    await wrapper.get("textarea").setValue("画一只猫");
    await wrapper.get(".agent-send-button").trigger("click");
    expect(wrapper.emitted("send")).toEqual([[{ content: "画一只猫", drawThisTurn: true }]]);
    await wrapper.get('button[aria-label="移除参考图"]').trigger("click");
    expect(wrapper.emitted("remove-attachment")).toEqual([["ref-1"]]);
    await wrapper.setProps({ busy: true });
    expect(wrapper.text()).toContain("停止");
  });
});
