use std::future::Future;

use serde_json::{json, Value};

use crate::{
    models::ApiProvider,
    models::{AgentEnvelope, AgentToolCall},
    services::{
        agent_tools::{parse_fallback_envelope, tool_definitions, validate_tool_arguments},
        chat::{agent_completion, agent_response, AgentModelResponse, ChatProgressEventData},
    },
    state::RuntimeState,
};

const MAX_TOOL_ROUNDS: usize = 8;

pub(crate) struct AgentTurnResult {
    pub text: String,
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
    for _round in 0..MAX_TOOL_ROUNDS {
        let response = match agent_completion(
            provider,
            messages,
            &tools,
            runtime_state,
            &mut on_event,
        )
        .await
        {
            Ok(response) => response,
            Err(error) if error.starts_with("AGENT_TOOLS_UNSUPPORTED:") => {
                let context = messages
                    .iter()
                    .map(|message| serde_json::to_string(message).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join("\n");
                let last_user = messages
                    .iter()
                    .rev()
                    .find(|message| message.get("role").and_then(Value::as_str) == Some("user"))
                    .and_then(|message| message.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let fallback = agent_response(
                    provider,
                    &format!(
                        "{}\n\n只能返回 schemaVersion=1 的 assistant/tool_call envelope，不得输出其它 JSON。",
                        context
                    ),
                    last_user,
                    runtime_state,
                    &mut on_event,
                )
                .await?;
                fallback_to_model_response(&fallback.text)?
            }
            Err(error) => return Err(error),
        };
        if response.tool_calls.is_empty() {
            return Ok(AgentTurnResult {
                text: response.text,
                tool_calls: completed_tool_calls,
            });
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

fn fallback_to_model_response(text: &str) -> Result<AgentModelResponse, String> {
    match parse_fallback_envelope(text)? {
        AgentEnvelope::Assistant {
            message, questions, ..
        } => Ok(AgentModelResponse {
            text: if questions.is_empty() {
                message
            } else {
                format!(
                    "{}\n{}",
                    message,
                    questions
                        .iter()
                        .map(|question| question.label.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            },
            tool_calls: Vec::new(),
            mode: "json-fallback".into(),
        }),
        AgentEnvelope::ToolCall {
            id,
            name,
            arguments,
            ..
        } => Ok(AgentModelResponse {
            text: String::new(),
            tool_calls: vec![crate::services::chat::AgentModelToolCall {
                id,
                name,
                arguments: serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".into()),
            }],
            mode: "json-fallback".into(),
        }),
        AgentEnvelope::ToolResult { .. } => Err("Agent 不应主动返回 tool_result".into()),
    }
}
