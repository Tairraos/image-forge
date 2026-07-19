use serde_json::{json, Value};

use crate::models::AgentEnvelope;

pub(crate) const TOOL_LIST_SKILLS: &str = "list_skills";
pub(crate) const TOOL_INSTALL_SKILL: &str = "install_skill";
pub(crate) const TOOL_USE_SKILL: &str = "use_skill";
pub(crate) const TOOL_CREATE_IMAGE_TASKS: &str = "create_image_tasks";
pub(crate) const TOOL_GET_TASK_STATUS: &str = "get_task_status";

pub(crate) fn tool_definitions() -> Vec<Value> {
    vec![
        function_tool(
            TOOL_LIST_SKILLS,
            "列出已安装的安全 Markdown Skill，只返回索引和能力摘要。",
            json!({
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "additionalProperties": false
            }),
        ),
        function_tool(
            TOOL_INSTALL_SKILL,
            "从用户明确提供的 HTTP(S)/GitHub URL 或本地包路径安装 Skill。安装前由 Rust 安全审查。",
            json!({
                "type": "object",
                "properties": {
                    "source": { "type": "string" },
                    "replace": { "type": "boolean", "default": false }
                },
                "required": ["source"],
                "additionalProperties": false
            }),
        ),
        function_tool(
            TOOL_USE_SKILL,
            "读取指定 Skill 的 SKILL.md、manifest 和 Markdown references，作为本轮规划规范。",
            json!({
                "type": "object",
                "properties": {
                    "skillId": { "type": "string" },
                    "task": { "type": "string" },
                    "referenceIds": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["skillId", "task"],
                "additionalProperties": false
            }),
        ),
        function_tool(
            TOOL_CREATE_IMAGE_TASKS,
            "把已经完整明确的单图或多图计划原子提交到绘画队列。",
            json!({
                "type": "object",
                "properties": {
                    "skillId": { "type": "string" },
                    "plans": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 12,
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "prompt": { "type": "string" },
                                "providerId": { "type": "string" },
                                "resolution": { "enum": ["standard", "2k", "4k"] },
                                "ratio": { "type": "string" },
                                "quality": { "enum": ["auto", "low", "medium", "high"] },
                                "promptFidelity": { "enum": ["original", "strict", "off"] },
                                "referencePolicy": { "enum": ["use", "optional", "none"] },
                                "referenceIds": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["title", "prompt", "referencePolicy", "referenceIds"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["plans"],
                "additionalProperties": false
            }),
        ),
        function_tool(
            TOOL_GET_TASK_STATUS,
            "只读查询一个任务组或单个绘图任务的当前状态。",
            json!({
                "type": "object",
                "properties": {
                    "taskGroupId": { "type": "string" },
                    "taskId": { "type": "string" }
                },
                "additionalProperties": false,
                "anyOf": [
                    { "required": ["taskGroupId"] },
                    { "required": ["taskId"] }
                ]
            }),
        ),
    ]
}

pub(crate) fn parse_fallback_envelope(text: &str) -> Result<AgentEnvelope, String> {
    let body = extract_json_body(text).ok_or("Agent 没有返回有效的 JSON envelope")?;
    let envelope: AgentEnvelope =
        serde_json::from_str(body).map_err(|error| format!("解析 Agent envelope 失败: {error}"))?;
    validate_envelope(&envelope)?;
    Ok(envelope)
}

pub(crate) fn validate_tool_arguments(name: &str, arguments: &Value) -> Result<(), String> {
    let object = arguments
        .as_object()
        .ok_or("Tool Call 参数必须是 JSON 对象")?;
    match name {
        TOOL_LIST_SKILLS => Ok(()),
        TOOL_INSTALL_SKILL => require_non_empty_string(object.get("source"), "source"),
        TOOL_USE_SKILL => {
            require_non_empty_string(object.get("skillId"), "skillId")?;
            require_non_empty_string(object.get("task"), "task")
        }
        TOOL_CREATE_IMAGE_TASKS => {
            let plans = object
                .get("plans")
                .and_then(Value::as_array)
                .ok_or("create_image_tasks.plans 必须是数组")?;
            if plans.is_empty() || plans.len() > 12 {
                return Err("create_image_tasks.plans 数量必须在 1 到 12 之间".into());
            }
            for (index, plan) in plans.iter().enumerate() {
                let plan = plan
                    .as_object()
                    .ok_or_else(|| format!("plans[{index}] 必须是对象"))?;
                require_non_empty_string(plan.get("prompt"), &format!("plans[{index}].prompt"))?;
                let policy = plan
                    .get("referencePolicy")
                    .and_then(Value::as_str)
                    .unwrap_or("optional");
                if !matches!(policy, "use" | "optional" | "none") {
                    return Err(format!("plans[{index}].referencePolicy 无效"));
                }
                let references = plan
                    .get("referenceIds")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0);
                if policy == "use" && references == 0 {
                    return Err(format!("plans[{index}] 要求使用参考图但没有 referenceIds"));
                }
                if policy == "none" && references > 0 {
                    return Err(format!("plans[{index}] 禁止参考图但仍提供了 referenceIds"));
                }
            }
            Ok(())
        }
        TOOL_GET_TASK_STATUS => {
            let has_group = non_empty_string(object.get("taskGroupId"));
            let has_task = non_empty_string(object.get("taskId"));
            if has_group || has_task {
                Ok(())
            } else {
                Err("get_task_status 需要 taskGroupId 或 taskId".into())
            }
        }
        _ => Err(format!("不允许的 Agent 工具：{name}")),
    }
}

fn validate_envelope(envelope: &AgentEnvelope) -> Result<(), String> {
    match envelope {
        AgentEnvelope::Assistant {
            schema_version,
            status,
            message,
            questions,
            plans,
            skill_id,
            skill_content_hash: _,
        } => {
            validate_schema_version(*schema_version)?;
            if questions.len() > 3 {
                return Err("assistant questions 最多 3 个".into());
            }
            for (index, question) in questions.iter().enumerate() {
                if question.key.trim().is_empty() || question.label.trim().is_empty() {
                    return Err(format!("questions[{index}] 必须包含 key 和 label"));
                }
            }
            match status.as_str() {
                "chat"
                    if !message.trim().is_empty() && questions.is_empty() && plans.is_empty() =>
                {
                    Ok(())
                }
                "needs_input" if !questions.is_empty() && plans.is_empty() => Ok(()),
                "rejected"
                    if !message.trim().is_empty() && questions.is_empty() && plans.is_empty() =>
                {
                    Ok(())
                }
                "ready" if questions.is_empty() && !plans.is_empty() => {
                    let arguments = json!({ "skillId": skill_id, "plans": plans });
                    validate_tool_arguments(TOOL_CREATE_IMAGE_TASKS, &arguments)
                }
                "chat" => Err("status=chat 必须只有非空 message".into()),
                "needs_input" => {
                    Err("status=needs_input 必须有 1-3 个 questions 且不能包含 plans".into())
                }
                "ready" => Err("status=ready 必须包含 plans 且不能包含 questions".into()),
                "rejected" => Err("status=rejected 必须只有非空拒绝原因".into()),
                _ => Err(format!("未知 assistant status：{status}")),
            }
        }
        AgentEnvelope::ToolCall {
            schema_version,
            name,
            arguments,
            ..
        } => {
            validate_schema_version(*schema_version)?;
            validate_tool_arguments(name, arguments)
        }
        AgentEnvelope::ToolResult { schema_version, .. } => {
            validate_schema_version(*schema_version)
        }
    }
}

fn validate_schema_version(value: u32) -> Result<(), String> {
    if value == 1 {
        Ok(())
    } else {
        Err(format!("不支持的 Agent schemaVersion：{value}"))
    }
}

fn function_tool(name: &str, description: &str, parameters: Value) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": parameters
        }
    })
}

fn require_non_empty_string(value: Option<&Value>, field: &str) -> Result<(), String> {
    if non_empty_string(value) {
        Ok(())
    } else {
        Err(format!("{field} 不能为空"))
    }
}

fn non_empty_string(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
}

fn extract_json_body(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if let Some(inner) = trimmed
        .strip_prefix("```json")
        .and_then(|value| value.strip_suffix("```"))
    {
        return Some(inner.trim());
    }
    if let Some(inner) = trimmed
        .strip_prefix("```")
        .and_then(|value| value.strip_suffix("```"))
    {
        return Some(inner.trim());
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    (end > start).then_some(trimmed[start..=end].trim())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{parse_fallback_envelope, tool_definitions, validate_tool_arguments};

    #[test]
    fn tool_registry_exposes_only_the_five_allowed_tools() {
        let tools = tool_definitions();
        let names = tools
            .iter()
            .filter_map(|tool| {
                tool.pointer("/function/name")
                    .and_then(|value| value.as_str())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "list_skills",
                "install_skill",
                "use_skill",
                "create_image_tasks",
                "get_task_status"
            ]
        );
    }

    #[test]
    fn image_plan_validation_enforces_reference_policy() {
        let error = validate_tool_arguments(
            "create_image_tasks",
            &json!({
                "plans": [{
                    "title": "图一",
                    "prompt": "完整提示词",
                    "referencePolicy": "use",
                    "referenceIds": []
                }]
            }),
        )
        .unwrap_err();
        assert!(error.contains("没有 referenceIds"));
    }

    #[test]
    fn fallback_protocol_requires_schema_version_one() {
        let error =
            parse_fallback_envelope(r#"{"schemaVersion":2,"type":"assistant","message":"hello"}"#)
                .unwrap_err();
        assert!(error.contains("schemaVersion"));
    }

    #[test]
    fn structured_skill_states_enforce_questions_and_complete_plans() {
        let needs_input = parse_fallback_envelope(
            r#"{"schemaVersion":1,"type":"assistant","status":"needs_input","message":"还需要信息","questions":[{"key":"style","label":"想要什么风格？","required":true}],"plans":[]}"#,
        )
        .unwrap();
        assert!(matches!(
            needs_input,
            crate::models::AgentEnvelope::Assistant { status, .. } if status == "needs_input"
        ));

        let rejected = parse_fallback_envelope(
            r#"{"schemaVersion":1,"type":"assistant","status":"rejected","message":"当前能力无法执行","questions":[],"plans":[]}"#,
        )
        .unwrap();
        assert!(matches!(
            rejected,
            crate::models::AgentEnvelope::Assistant { status, .. } if status == "rejected"
        ));

        let ready = parse_fallback_envelope(
            r#"{"schemaVersion":1,"type":"assistant","status":"ready","message":"计划完成","questions":[],"plans":[{"title":"图一","prompt":"完整提示词","resolution":"standard","ratio":"1:1","quality":"auto","promptFidelity":"original","referencePolicy":"none","referenceIds":[]}]}"#,
        )
        .unwrap();
        assert!(matches!(
            ready,
            crate::models::AgentEnvelope::Assistant { status, .. } if status == "ready"
        ));

        let error = parse_fallback_envelope(
            r#"{"schemaVersion":1,"type":"assistant","status":"ready","message":"计划完成","questions":[],"plans":[]}"#,
        )
        .unwrap_err();
        assert!(error.contains("status=ready"));
    }
}
