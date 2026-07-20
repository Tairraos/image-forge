import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import AgentWorkspace from "../../src/components/AgentWorkspace.vue";

describe("AgentWorkspace", () => {
  it("选择、删除会话并转发任务组跳转", async () => {
    const sessions = [
      { id: "old", title: "较早对话", createdAt: "2026-07-19T08:00:00Z" },
      { id: "new", title: "较晚对话", createdAt: "2026-07-20T08:00:00Z" },
    ];
    const wrapper = mount(AgentWorkspace, {
      props: { sessions, currentSession: sessions[0], providerId: "chat", imageProviderId: "image" },
      global: {
        stubs: {
          AgentComposer: true,
          AgentMessageList: {
            emits: ["open-task-group"],
            template: '<button class="open-group" @click="$emit(\'open-task-group\', { id: \'group-1\' })">打开</button>',
          },
        },
      },
    });
    expect(wrapper.findAll(".agent-session-title").map((item) => item.text())).toEqual(["较早对话", "较晚对话"]);
    await wrapper.findAll(".agent-session-item")[1].trigger("click");
    expect(wrapper.emitted("select")).toEqual([["new"]]);
    await wrapper.findAll(".agent-session-delete")[0].trigger("click");
    expect(wrapper.emitted("delete-session")).toEqual([["old"]]);
    await wrapper.get(".open-group").trigger("click");
    expect(wrapper.emitted("open-task-group")).toEqual([[{ id: "group-1" }]]);
  });

  it("同时提供生图与对话模型选择", async () => {
    const wrapper = mount(AgentWorkspace, {
      props: {
        providerId: "chat-1",
        providerOptions: [{ label: "对话模型", value: "chat-1" }],
        imageProviderId: "image-1",
        imageProviderOptions: [{ label: "生图模型", value: "image-1" }],
      },
      global: { stubs: { AgentComposer: true, AgentMessageList: true } },
    });
    const selects = wrapper.findAll(".agent-provider-select");
    expect(selects).toHaveLength(2);
    await wrapper.setProps({
      imageProviderOptions: [{ label: "生图模型 2", value: "image-2" }],
      providerOptions: [{ label: "对话模型 2", value: "chat-2" }],
    });
    await selects[0].setValue("image-2");
    await selects[1].setValue("chat-2");
    expect(wrapper.emitted("update:image-provider-id")).toEqual([["image-2"]]);
    expect(wrapper.emitted("update:provider-id")).toEqual([["chat-2"]]);
  });
});
