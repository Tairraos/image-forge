use std::future::Future;

use serde_json::{json, Value};

use crate::{
    models::ApiProvider,
    models::{AgentEnvelope, AgentQuestion, AgentToolCall},
    services::{
        agent_tools::{parse_fallback_envelope, tool_definitions, validate_tool_arguments},
        chat::{
            agent_completion, agent_fallback_response, AgentModelResponse, AgentModelToolCall,
            ChatProgressEventData,
        },
    },
    state::RuntimeState,
};

const MAX_TOOL_ROUNDS: usize = 8;

pub(crate) struct AgentTurnResult {
    pub text: String,
    pub status: String,
    pub questions: Vec<AgentQuestion>,
    pub skill_id: String,
    pub skill_content_hash: String,
    pub tool_calls: Vec<AgentToolCall>,
}

pub(crate) async fn run_turn<F, E, Fut>(
    provider: &ApiProvider,
    messages: &mut Vec<Value>,
    runtime_state: Option<&RuntimeState>,
    mut execute_tool: E,
    mut on_event: F,
) -> Result<AgentTurnResult, String>
where
    F: FnMut(ChatProgressEventData),
    E: FnMut(String, Value) -> Fut,
    Fut: Future<Output = Result<Value, String>>,
{
    let tools = tool_definitions();
    let mut completed_tool_calls = Vec::new();
    let mut fallback_mode = false;
    for _round in 0..MAX_TOOL_ROUNDS {
        let mut response = if fallback_mode {
            let text = agent_fallback_response(
                provider,
                messages,
                &tools,
                None,
                runtime_state,
                &mut on_event,
            )
            .await?;
            let envelope = parse_or_repair_envelope(
                provider,
                messages,
                &tools,
                &text,
                runtime_state,
                &mut on_event,
            )
            .await?;
            match envelope_step(envelope, &completed_tool_calls)? {
                EnvelopeStep::Final(result) => return Ok(result),
                EnvelopeStep::Continue(response) => response,
            }
        } else {
            match agent_completion(provider, messages, &tools, runtime_state, &mut on_event).await {
                Ok(response) => response,
                Err(error) if error.starts_with("AGENT_TOOLS_UNSUPPORTED:") => {
                    fallback_mode = true;
                    continue;
                }
                Err(error) => return Err(error),
            }
        };
        if response.tool_calls.is_empty() && looks_like_envelope(&response.text) {
            let envelope = parse_or_repair_envelope(
                provider,
                messages,
                &tools,
                &response.text,
                runtime_state,
                &mut on_event,
            )
            .await?;
            match envelope_step(envelope, &completed_tool_calls)? {
                EnvelopeStep::Final(result) => return Ok(result),
                EnvelopeStep::Continue(next) => response = next,
            }
        }
        if response.tool_calls.is_empty() {
            return Ok(final_result(
                response.text,
                "chat".into(),
                Vec::new(),
                String::new(),
                String::new(),
                completed_tool_calls,
            ));
        }
        let tool_calls = response.tool_calls.clone();
        let parsed_calls = tool_calls
            .into_iter()
            .map(|call| {
                let arguments: Value = serde_json::from_str(&call.arguments).map_err(|error| {
                    format!("Tool Call {} 参数不是有效 JSON: {error}", call.name)
                })?;
                validate_tool_arguments(&call.name, &arguments)?;
                Ok((call, arguments))
            })
            .collect::<Result<Vec<_>, String>>()?;
        messages.push(json!({
            "role": "assistant",
            "content": if response.text.trim().is_empty() { Value::Null } else { Value::String(response.text.clone()) },
            "tool_calls": parsed_calls.iter().map(|(call, _)| json!({
                "id": call.id,
                "type": "function",
                "function": { "name": call.name, "arguments": call.arguments }
            })).collect::<Vec<_>>(),
        }));
        for (call, arguments) in parsed_calls {
            on_event(ChatProgressEventData {
                phase: "tool_start",
                mode: mode_label(&response.mode),
                chunk: String::new(),
                message: format!("正在执行 {}", call.name),
            });
            let result = execute_tool(call.name.clone(), arguments.clone()).await;
            let (result_value, error) = match result {
                Ok(value) => (value, String::new()),
                Err(error) => (Value::Null, error),
            };
            let tool_result_payload =
                json!({ "result": result_value.clone(), "error": error.clone() });
            messages.push(json!({
                "role": "tool",
                "tool_call_id": call.id.clone(),
                "name": call.name.clone(),
                "content": serde_json::to_string(&tool_result_payload).unwrap_or_default(),
            }));
            completed_tool_calls.push(AgentToolCall {
                id: call.id.clone(),
                name: call.name.clone(),
                arguments: arguments.clone(),
                result: Some(result_value),
                error: if error.is_empty() {
                    None
                } else {
                    Some(error.clone())
                },
                status: if error.is_empty() {
                    "completed".into()
                } else {
                    "failed".into()
                },
                created_at: crate::utils::utc_now(),
                completed_at: Some(crate::utils::utc_now()),
            });
            on_event(ChatProgressEventData {
                phase: "tool_result",
                mode: mode_label(&response.mode),
                chunk: String::new(),
                message: if error.is_empty() {
                    "工具执行完成".into()
                } else {
                    error
                },
            });
        }
    }
    Err("Agent Tool Call 超过最大循环次数，已停止本轮".into())
}

fn mode_label(mode: &str) -> &'static str {
    match mode {
        "json-fallback" => "json-fallback",
        "non-stream" => "non-stream",
        _ => "stream",
    }
}

enum EnvelopeStep {
    Final(AgentTurnResult),
    Continue(AgentModelResponse),
}

async fn parse_or_repair_envelope<F>(
    provider: &ApiProvider,
    messages: &[Value],
    tools: &[Value],
    text: &str,
    runtime_state: Option<&RuntimeState>,
    on_event: &mut F,
) -> Result<AgentEnvelope, String>
where
    F: FnMut(ChatProgressEventData),
{
    match parse_fallback_envelope(text) {
        Ok(envelope) => Ok(envelope),
        Err(first_error) => {
            let repaired = agent_fallback_response(
                provider,
                messages,
                tools,
                Some((text, &first_error)),
                runtime_state,
                on_event,
            )
            .await?;
            parse_fallback_envelope(&repaired).map_err(|second_error| {
                format!(
                    "Agent 返回结构无效，自动修复后仍无法解析：{second_error}（首次错误：{first_error}）"
                )
            })
        }
    }
}

fn envelope_step(
    envelope: AgentEnvelope,
    completed_tool_calls: &[AgentToolCall],
) -> Result<EnvelopeStep, String> {
    match envelope {
        AgentEnvelope::Assistant {
            status,
            message,
            questions: _,
            plans,
            skill_id,
            skill_content_hash: _,
            ..
        } if status == "ready" => {
            let arguments = json!({ "skillId": skill_id, "plans": plans });
            validate_tool_arguments("create_image_tasks", &arguments)?;
            Ok(EnvelopeStep::Continue(AgentModelResponse {
                text: message,
                tool_calls: vec![AgentModelToolCall {
                    id: format!("call-{}", uuid::Uuid::new_v4()),
                    name: "create_image_tasks".into(),
                    arguments: serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".into()),
                }],
                mode: "json-fallback".into(),
            }))
        }
        AgentEnvelope::Assistant {
            status,
            message,
            questions,
            skill_id,
            skill_content_hash,
            ..
        } => Ok(EnvelopeStep::Final(final_result(
            message,
            status,
            questions,
            skill_id,
            skill_content_hash,
            completed_tool_calls.to_vec(),
        ))),
        AgentEnvelope::ToolCall {
            id,
            name,
            arguments,
            ..
        } => Ok(EnvelopeStep::Continue(AgentModelResponse {
            text: String::new(),
            tool_calls: vec![AgentModelToolCall {
                id: if id.trim().is_empty() {
                    format!("call-{}", uuid::Uuid::new_v4())
                } else {
                    id
                },
                name,
                arguments: serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".into()),
            }],
            mode: "json-fallback".into(),
        })),
        AgentEnvelope::ToolResult { .. } => Err("Agent 不应主动返回 tool_result".into()),
    }
}

fn final_result(
    text: String,
    status: String,
    questions: Vec<AgentQuestion>,
    mut skill_id: String,
    mut skill_content_hash: String,
    tool_calls: Vec<AgentToolCall>,
) -> AgentTurnResult {
    if skill_id.trim().is_empty() || skill_content_hash.trim().is_empty() {
        for call in tool_calls
            .iter()
            .rev()
            .filter(|call| call.name == "use_skill")
        {
            let Some(result) = call.result.as_ref() else {
                continue;
            };
            if skill_id.trim().is_empty() {
                skill_id = result
                    .get("skillId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
            }
            if skill_content_hash.trim().is_empty() {
                skill_content_hash = result
                    .pointer("/manifest/contentHash")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
            }
        }
    }
    AgentTurnResult {
        text,
        status,
        questions,
        skill_id,
        skill_content_hash,
        tool_calls,
    }
}

fn looks_like_envelope(text: &str) -> bool {
    let text = text.trim_start();
    text.starts_with('{') || text.starts_with("```json") || text.starts_with("```")
}
