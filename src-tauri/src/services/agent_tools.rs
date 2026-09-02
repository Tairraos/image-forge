use serde_json::{json, Value};

use crate::models::{AgentEnvelope, AGENT_SCHEMA_VERSION};

pub(crate) const TOOL_CREATE_IMAGE_TASKS: &str = "create_image_tasks";
pub(crate) const TOOL_GET_TASK_STATUS: &str = "get_task_status";
pub(crate) const TOOL_LIST_TEMPLATES: &str = "list_templates";

pub(crate) fn tool_definitions() -> Vec<Value> {
    vec![
        function_tool(
            TOOL_CREATE_IMAGE_TASKS,
            "把已经完整明确的单图或多图计划原子提交到绘画队列。",
            json!({
                "type": "object",
                "properties": {
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
                                "resolution": { "enum": ["standard", "2k", "3k", "4k"] },
                                "ratio": { "type": "string" },
                                "quality": { "enum": ["auto", "low", "medium", "high"] },
                                "promptFidelity": { "enum": ["original", "strict", "off"] },
                                "referencePolicy": { "enum": ["use", "optional", "none"] },
                                "referenceIds": { "type": "array", "items": { "type": "string" } },
                                "templateId": { "type": "string" }
                            },
                            "required": [
                                "title", "prompt", "resolution", "ratio", "quality",
                                "promptFidelity", "referencePolicy", "referenceIds"
                            ],
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
        function_tool(
            TOOL_LIST_TEMPLATES,
            "只读列出本机提示词模板（id、标题、内容摘要、参考图数量）。用户要求使用模板绘画时先查询，再把模板 id 填入计划的 templateId。",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
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
        TOOL_CREATE_IMAGE_TASKS => {
            reject_unknown_fields(name, object, &["plans"])?;
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
                reject_unknown_fields(
                    &format!("plans[{index}]"),
                    plan,
                    &[
                        "title",
                        "prompt",
                        "providerId",
                        "resolution",
                        "ratio",
                        "quality",
                        "promptFidelity",
                        "referencePolicy",
                        "referenceIds",
                        "templateId",
                    ],
                )?;
                let label = format!("plans[{index}]");
                require_non_empty_string(plan.get("title"), &format!("{label}.title"))?;
                require_non_empty_string(plan.get("prompt"), &format!("{label}.prompt"))?;
                require_enum(
                    plan.get("resolution"),
                    &format!("{label}.resolution"),
                    &["standard", "2k", "3k", "4k"],
                )?;
                require_non_empty_string(plan.get("ratio"), &format!("{label}.ratio"))?;
                require_enum(
                    plan.get("quality"),
                    &format!("{label}.quality"),
                    &["auto", "low", "medium", "high"],
                )?;
                require_enum(
                    plan.get("promptFidelity"),
                    &format!("{label}.promptFidelity"),
                    &["original", "strict", "off"],
                )?;
                let policy = require_enum(
                    plan.get("referencePolicy"),
                    &format!("{label}.referencePolicy"),
                    &["use", "optional", "none"],
                )?;
                let reference_ids = plan
                    .get("referenceIds")
                    .and_then(Value::as_array)
                    .ok_or_else(|| format!("{label}.referenceIds 必须是数组"))?;
                if reference_ids
                    .iter()
                    .any(|value| !non_empty_string(Some(value)))
                {
                    return Err(format!("{label}.referenceIds 只能包含非空字符串"));
                }
                if policy == "use" && reference_ids.is_empty() {
                    return Err(format!("plans[{index}] 要求使用参考图但没有 referenceIds"));
                }
                if policy == "none" && !reference_ids.is_empty() {
                    return Err(format!("plans[{index}] 禁止参考图但仍提供了 referenceIds"));
                }
                if let Some(template_id) = plan.get("templateId") {
                    if template_id.as_str().is_none() {
                        return Err(format!("{label}.templateId 必须是字符串"));
                    }
                }
            }
            Ok(())
        }
        TOOL_GET_TASK_STATUS => {
            reject_unknown_fields(name, object, &["taskGroupId", "taskId"])?;
            let has_group = non_empty_string(object.get("taskGroupId"));
            let has_task = non_empty_string(object.get("taskId"));
            if has_group || has_task {
                Ok(())
            } else {
                Err("get_task_status 需要 taskGroupId 或 taskId".into())
            }
        }
        TOOL_LIST_TEMPLATES => {
            reject_unknown_fields(name, object, &[])?;
            Ok(())
        }
        _ => Err(format!("不允许的 Agent 工具：{name}")),
    }
}

fn reject_unknown_fields(
    label: &str,
    object: &serde_json::Map<String, Value>,
    allowed: &[&str],
) -> Result<(), String> {
    if let Some(key) = object
        .keys()
        .find(|key| !allowed.iter().any(|field| field == &key.as_str()))
    {
        Err(format!("{label} 不允许未知字段 `{key}`"))
    } else {
        Ok(())
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
                    let arguments = json!({ "plans": plans });
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
    if value == AGENT_SCHEMA_VERSION {
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

fn require_enum<'a>(
    value: Option<&'a Value>,
    field: &str,
    allowed: &[&str],
) -> Result<&'a str, String> {
    let value = value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{field} 不能为空"))?;
    if allowed.contains(&value) {
        Ok(value)
    } else {
        Err(format!("{field} 无效"))
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

    fn valid_image_plan() -> serde_json::Value {
        json!({
            "title": "图一",
            "prompt": "完整提示词",
            "resolution": "standard",
            "ratio": "1:1",
            "quality": "auto",
            "promptFidelity": "original",
            "referencePolicy": "none",
            "referenceIds": []
        })
    }

    #[test]
    fn tool_registry_exposes_image_tasks_status_and_templates() {
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
            vec!["create_image_tasks", "get_task_status", "list_templates"]
        );
        let required = tools
            .iter()
            .find(|tool| tool.pointer("/function/name") == Some(&json!("create_image_tasks")))
            .and_then(|tool| tool.pointer("/function/parameters/properties/plans/items/required"))
            .and_then(|value| value.as_array())
            .unwrap();
        for field in [
            "title",
            "prompt",
            "resolution",
            "ratio",
            "quality",
            "promptFidelity",
            "referencePolicy",
            "referenceIds",
        ] {
            assert!(required.iter().any(|value| value == field));
        }
    }

    #[test]
    fn image_plan_validation_enforces_reference_policy() {
        let mut plan = valid_image_plan();
        plan["referencePolicy"] = json!("use");
        let error =
            validate_tool_arguments("create_image_tasks", &json!({ "plans": [plan] })).unwrap_err();
        assert!(error.contains("没有 referenceIds"));
    }

    #[test]
    fn image_plan_validation_rejects_missing_fields_invalid_enums_and_reference_types() {
        let mut plan = valid_image_plan();
        plan.as_object_mut().unwrap().remove("resolution");
        let missing =
            validate_tool_arguments("create_image_tasks", &json!({ "plans": [plan] })).unwrap_err();
        assert!(missing.contains("resolution"));

        let mut plan = valid_image_plan();
        plan["resolution"] = json!("8k");
        let invalid =
            validate_tool_arguments("create_image_tasks", &json!({ "plans": [plan] })).unwrap_err();
        assert!(invalid.contains("resolution 无效"));

        let mut plan = valid_image_plan();
        plan["referencePolicy"] = json!("optional");
        plan["referenceIds"] = json!([" "]);
        let invalid_references =
            validate_tool_arguments("create_image_tasks", &json!({ "plans": [plan] })).unwrap_err();
        assert!(invalid_references.contains("referenceIds 只能包含非空字符串"));
    }

    #[test]
    fn tool_validation_rejects_unknown_fields() {
        let mut plan = valid_image_plan();
        plan["unexpected"] = json!(true);
        let error =
            validate_tool_arguments("create_image_tasks", &json!({ "plans": [plan] })).unwrap_err();
        assert!(error.contains("未知字段"));
    }

    #[test]
    fn fallback_protocol_requires_schema_version_one() {
        let error =
            parse_fallback_envelope(r#"{"schemaVersion":2,"type":"assistant","message":"hello"}"#)
                .unwrap_err();
        assert!(error.contains("schemaVersion"));
    }

    #[test]
    fn structured_agent_states_enforce_questions_and_complete_plans() {
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
