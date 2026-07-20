import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { describe, expect, it } from "vitest";
import AgentMessageList from "../../src/components/AgentMessageList.vue";

const baseMessage = {
  id: "message-1",
  role: "assistant",
  content: "**完成**",
  createdAt: "2026-07-20T08:00:00Z",
};

describe("AgentMessageList", () => {
  it("渲染 Markdown、流式消息和工具状态", () => {
    const wrapper = mount(AgentMessageList, {
      props: {
        messages: [baseMessage, {
          id: "tool-1",
          role: "tool",
          content: "",
          toolCall: { name: "create_image_tasks", status: "running" },
        }],
        busy: true,
        streamText: "正在**生成**",
      },
    });
    expect(wrapper.html()).toContain("<strong>完成</strong>");
    expect(wrapper.html()).toContain("<strong>生成</strong>");
    expect(wrapper.text()).toContain("create_image_tasks");
    expect(wrapper.text()).toContain("执行中");
    expect(wrapper.text()).toContain("工具");
  });

  it("隐藏成功工具的原始结果，只显示可换行的错误", () => {
    const wrapper = mount(AgentMessageList, {
      props: {
        messages: [
          {
            id: "skills",
            role: "tool",
            content: '{"error":null,"result":[]}',
            toolCall: { name: "list_skills", status: "completed", error: null },
          },
          {
            id: "failed",
            role: "tool",
            content: '{"error":"请求失败","result":null}',
            toolCall: { name: "use_skill", status: "failed", error: "很长的错误信息" },
          },
        ],
      },
    });
    expect(wrapper.text()).not.toContain('{"error"');
    expect(wrapper.get(".agent-tool-card.compact").text()).toContain("list_skills");
    expect(wrapper.text()).toContain("很长的错误信息");
  });

  it("提交交互问题并打开任务组", async () => {
    const questionMessage = {
      ...baseMessage,
      id: "question",
      questions: [{ key: "style", label: "风格", placeholder: "请输入" }],
    };
    const taskGroup = { id: "group-1", taskIds: ["task-1"], status: "completed" };
    const wrapper = mount(AgentMessageList, {
      props: { messages: [questionMessage, { ...baseMessage, id: "group", taskGroup }] },
    });
    await wrapper.get("textarea").setValue("水彩");
    expect(wrapper.emitted("update-answer")).toEqual([[{ key: "style", value: "水彩" }]]);
    await wrapper.get(".agent-question-actions button").trigger("click");
    expect(wrapper.emitted("answer-questions")[0][0].id).toBe("question");
    await wrapper.get(".agent-task-group-open").trigger("click");
    expect(wrapper.emitted("open-task-group")).toEqual([[taskGroup]]);
  });

  it("仅在接近底部时自动跟随流式内容", async () => {
    const wrapper = mount(AgentMessageList, { props: { messages: [baseMessage] } });
    const list = wrapper.element;
    Object.defineProperties(list, {
      scrollHeight: { configurable: true, value: 1000 },
      clientHeight: { configurable: true, value: 400 },
    });
    list.scrollTop = 100;
    await wrapper.setProps({ streamText: "远离底部" });
    await nextTick();
    expect(list.scrollTop).toBe(100);
    list.scrollTop = 520;
    await wrapper.setProps({ streamText: "接近底部" });
    await nextTick();
    expect(list.scrollTop).toBe(1000);
  });
});
